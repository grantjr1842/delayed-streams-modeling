// Copyright (c) Kyutai, all rights reserved.
// This source code is licensed under the license found in the
// LICENSE file in the root directory of this source tree.

use candle::{DType, Device, Result, Tensor};
use candle_nn::kv_cache::RotatingKvCache;

#[derive(Debug, Clone)]
pub struct IndicesAndMask {
    indices: Tensor,
    mask: Tensor,
}

impl IndicesAndMask {
    pub fn mask(&self) -> &Tensor {
        &self.mask
    }
}

#[derive(Debug, Clone)]
pub struct ScatteredKvCache {
    k: Tensor,
    v: Tensor,
    context: usize,
}

impl ScatteredKvCache {
    pub fn append(
        &mut self,
        k: &Tensor,
        v: &Tensor,
        iam: &IndicesAndMask,
    ) -> Result<(Tensor, Tensor)> {
        if self.context <= k.dim(2)? {
            return Ok((k.clone(), v.clone()));
        }
        let indices = iam.indices.unsqueeze(2)?.unsqueeze(1)?;
        let indices = indices.broadcast_as(k.shape())?;
        self.k.scatter_set(&indices, k, 2)?;
        self.v.scatter_set(&indices, v, 2)?;
        Ok((self.k.clone(), self.v.clone()))
    }

    pub fn k(&self) -> &Tensor {
        &self.k
    }

    pub fn v(&self) -> &Tensor {
        &self.v
    }
}

#[derive(Debug, Clone)]
pub struct ScatteredCacheBuilder {
    context: usize,
    // The current position in the stream, this can be larger than context.
    positions: Tensor,
    // The index where the next element will be stored.
    indices: Tensor,
    dtype: DType,
    device: Device,
    arange: Tensor,
    negative_inf: Tensor,
    zero: Tensor,
    batch_size: usize,
}

impl ScatteredCacheBuilder {
    pub fn new(batch_size: usize, context: usize, dtype: DType, device: &Device) -> Result<Self> {
        let positions = Tensor::zeros((batch_size,), DType::U32, device)?;
        let indices = Tensor::zeros((batch_size,), DType::U32, device)?;
        let arange = Tensor::arange(0u32, context as u32, device)?.unsqueeze(0)?;
        let negative_inf = Tensor::new(f32::NEG_INFINITY, device)?.to_dtype(dtype)?;
        let zero = Tensor::new(0.0f32, device)?.to_dtype(dtype)?;
        Ok(Self {
            positions,
            indices,
            context,
            dtype,
            device: device.clone(),
            arange,
            negative_inf,
            zero,
            batch_size,
        })
    }

    pub fn make_cache(&self, num_heads: usize, head_dim: usize) -> Result<ScatteredKvCache> {
        let batch_size = self.batch_size();
        let shape = (batch_size, num_heads, self.context, head_dim);
        let k = Tensor::zeros(shape, self.dtype, &self.device)?;
        let v = Tensor::zeros(shape, self.dtype, &self.device)?;
        Ok(ScatteredKvCache { k, v, context: self.context })
    }

    pub fn positions(&self) -> &Tensor {
        &self.positions
    }

    pub fn reset(&mut self) -> Result<()> {
        let z = self.positions.zeros_like()?;
        self.positions = z.clone();
        self.indices = z;
        Ok(())
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    pub fn reset_batch_index(&mut self, batch_index: usize) -> Result<()> {
        use candle::IndexOp;
        let z = self.positions.i(batch_index)?.zeros_like()?;
        self.positions.slice_set(&z, 0, batch_index)?;
        self.indices.slice_set(&z, 0, batch_index)?;
        Ok(())
    }

    pub fn indices_and_mask(
        &mut self,
        seq_len: usize,
        batch_mask: &[bool],
    ) -> Result<IndicesAndMask> {
        let context = self.context;
        if context <= seq_len {
            return self.indices_and_mask_abs(seq_len, batch_mask);
        }

        // Fast path for seq_len == 1 (common in streaming ASR)
        if seq_len == 1 {
            let b = self.batch_size();
            let active_vec: Vec<u32> = batch_mask.iter().map(|&b| if b { 1 } else { 0 }).collect();
            let active = Tensor::from_vec(active_vec, (b,), &self.device)?;
            let zero_u32 = Tensor::new(0u32, &self.device)?.broadcast_as(active.shape())?;
            let active_mask = active.ne(&zero_u32)?;

            let prev_indices = self.indices.clone();
            let prev_positions = self.positions.clone();

            let one = Tensor::new(1u32, &self.device)?;
            let next_indices = self.indices.broadcast_add(&one)?;

            let context_t = Tensor::new(context as u32, &self.device)?.broadcast_as(active.shape())?;
            let wrap_mask = next_indices.ge(&context_t)?.to_dtype(DType::U32)?;
            let sub_val = wrap_mask.broadcast_mul(&context_t)?; // U32 * U32
            let next_indices = next_indices.broadcast_sub(&sub_val)?;

            let next_positions = self.positions.broadcast_add(&one)?;

            self.indices = active_mask.where_cond(&next_indices, &self.indices)?;
            self.positions = active_mask.where_cond(&next_positions, &self.positions)?;

            let indices = active_mask.where_cond(&prev_indices, &self.indices)?;
            let start_pos_t = active_mask.where_cond(&prev_positions, &self.positions)?;

            // If a value is not active, we want the mask to be full zeros so we artifically
            // change the start_pos to be u32::MAX.
            let inactive_mask = active.eq(&zero_u32)?;
            let max_val = Tensor::new(u32::MAX, &self.device)?.broadcast_as(start_pos_t.shape())?;
            let start_pos_t = inactive_mask.where_cond(&max_val, &start_pos_t)?;

            let indices = indices.unsqueeze(1)?;
            let start_pos_t = start_pos_t.unsqueeze(1)?;

            let mask_bool = self.arange.broadcast_gt(&start_pos_t)?.unsqueeze(1)?.unsqueeze(1)?;
            let negative_inf = self.negative_inf.broadcast_as(mask_bool.shape())?;
            let zero = self.zero.broadcast_as(mask_bool.shape())?;
            let mask = mask_bool.where_cond(&negative_inf, &zero)?;
            return Ok(IndicesAndMask { indices, mask });
        }

        self.indices_and_mask_slow(seq_len, batch_mask)
    }

    #[allow(clippy::needless_range_loop)]
    fn indices_and_mask_slow(
        &mut self,
        seq_len: usize,
        batch_mask: &[bool],
    ) -> Result<IndicesAndMask> {
        let b = self.batch_size();
        let context = self.context;
        // This is slow as we move back to CPU.
        let indices_cpu = self.indices.to_vec1::<u32>()?;
        let positions_cpu = self.positions.to_vec1::<u32>()?;

        let mut attention_masks = Vec::with_capacity(b * seq_len * context);
        let mut cache_indices = Vec::with_capacity(b * seq_len);
        let mut next_indices = indices_cpu.clone();
        let mut next_positions = positions_cpu.clone();

        for (batch_i, &active) in batch_mask.iter().enumerate() {
            if !active {
                attention_masks.extend(std::iter::repeat_n(0.0, seq_len * context));
                cache_indices.extend(std::iter::repeat_n(indices_cpu[batch_i], seq_len));
            } else {
                let start_index = next_indices[batch_i] as usize;
                let start_pos = next_positions[batch_i] as usize;
                let mut all_pos = vec![usize::MAX; context];
                if start_pos < context {
                    for i in 0..start_pos {
                        all_pos[i] = i;
                    }
                } else {
                    let offset = start_pos - start_index;
                    for i in 0..context {
                        all_pos[i] =
                            if i < start_index { i + offset } else { i + offset - context };
                    }
                }
                for seq_i in 0..seq_len {
                    let index = next_indices[batch_i] as usize;
                    all_pos[index] = seq_i + start_pos;
                    cache_indices.push(index as u32);
                    next_indices[batch_i] += 1;
                    next_positions[batch_i] += 1;
                    if next_indices[batch_i] as usize >= context {
                        next_indices[batch_i] = 0;
                    }
                }

                for seq_i in 0..seq_len {
                    let my_pos = seq_i + start_pos;
                    attention_masks.extend(all_pos.iter().map(|&pos| {
                        if pos <= my_pos { 0.0 } else { f32::NEG_INFINITY }
                    }));
                }
            }
        }
        self.indices = Tensor::from_vec(next_indices, (b,), &self.device)?;
        self.positions = Tensor::from_vec(next_positions, (b,), &self.device)?;

        let mask = Tensor::from_vec(attention_masks, (b, 1, seq_len, context), &self.device)?
            .to_dtype(self.dtype)?;
        let indices = Tensor::from_vec(cache_indices, (b, seq_len), &self.device)?;
        Ok(IndicesAndMask { indices, mask })
    }

    #[allow(clippy::needless_range_loop)]
    fn indices_and_mask_abs(
        &mut self,
        seq_len: usize,
        batch_mask: &[bool],
    ) -> Result<IndicesAndMask> {
        let mask = self.get_mask_abs(seq_len, seq_len)?;
        let b = self.batch_size();
        let mut cache_indices = Vec::with_capacity(b);

        // This is slow as we move back to CPU.
        let indices_cpu = self.indices.to_vec1::<u32>()?;
        let positions_cpu = self.positions.to_vec1::<u32>()?;
        let mut next_indices = indices_cpu.clone();
        let mut next_positions = positions_cpu.clone();

        for (batch_i, &active) in batch_mask.iter().enumerate() {
            if !active {
                let indices = vec![next_indices[batch_i]; seq_len];
                cache_indices.push(indices);
            } else {
                let mut indices = Vec::with_capacity(seq_len);
                for _ in 0..seq_len {
                    let index = next_indices[batch_i];
                    indices.push(index);
                    next_indices[batch_i] += 1;
                    next_positions[batch_i] += 1;
                    if next_indices[batch_i] as usize >= self.context {
                        next_indices[batch_i] = 0;
                    }
                }
                cache_indices.push(indices);
            }
        }
        self.indices = Tensor::from_vec(next_indices, (b,), &self.device)?;
        self.positions = Tensor::from_vec(next_positions, (b,), &self.device)?;

        let indices = Tensor::new(cache_indices, &self.device)?;
        Ok(IndicesAndMask { indices, mask })
    }

    fn get_mask_abs(&self, size1: usize, size2: usize) -> Result<Tensor> {
        let context = self.context;
        let mask: Vec<_> = (0..size1)
            .flat_map(|_i| {
                (0..size2).map(move |j| {
                    if size1 + j > size2 + _i || size1 + j + context < size2 + _i {
                        f32::NEG_INFINITY
                    } else {
                        0.0
                    }
                })
            })
            .collect();
        Tensor::from_slice(&mask, (size1, size2), &self.device)
    }
}

#[derive(Debug, Clone)]
pub enum KvCache {
    Rotating(RotatingKvCache),
}

impl KvCache {
    pub fn new(dim: usize, max_seq_len: usize) -> Self {
        let cache = RotatingKvCache::new(dim, max_seq_len);
        Self::Rotating(cache)
    }

    pub fn current_seq_len(&self) -> usize {
        match self {
            KvCache::Rotating(cache) => cache.current_seq_len(),
        }
    }

    pub fn reset(&mut self) {
        match self {
            KvCache::Rotating(cache) => cache.reset(),
        }
    }

    pub fn append(&mut self, key: &Tensor, value: &Tensor) -> Result<(Tensor, Tensor)> {
        match self {
            KvCache::Rotating(cache) => cache.append(key, value),
        }
    }

    pub fn positions(&self, seq_len: usize) -> Vec<usize> {
        match self {
            KvCache::Rotating(cache) => cache.positions(seq_len),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle::IndexOp;

    #[test]
    fn test_scattered_kv_cache() -> Result<()> {
        let device = Device::Cpu;
        let mut cache = ScatteredCacheBuilder::new(2, 5, DType::F32, &device)?;
        let inf = f32::INFINITY;

        let iam = cache.indices_and_mask(1, &[true, false])?;
        let mask = iam.mask.i((.., 0))?.to_vec3::<f32>()?;
        assert_eq!(iam.indices.to_vec2::<u32>()?, [[0], [0]]);
        assert_eq!(mask, [[[0.0, -inf, -inf, -inf, -inf]], [[0.0, 0.0, 0.0, 0.0, 0.0]]]);

        let iam = cache.indices_and_mask(1, &[true, false])?;
        let mask = iam.mask.i((.., 0))?.to_vec3::<f32>()?;
        assert_eq!(iam.indices.to_vec2::<u32>()?, [[1], [0]]);
        assert_eq!(mask, [[[0.0, 0.0, -inf, -inf, -inf]], [[0.0, 0.0, 0.0, 0.0, 0.0]]]);

        let iam = cache.indices_and_mask(3, &[false, true])?;
        let mask = iam.mask.i((.., 0))?.to_vec3::<f32>()?;
        assert_eq!(iam.indices.to_vec2::<u32>()?, [[2, 2, 2], [0, 1, 2]]);
        assert_eq!(
            mask,
            [
                [[0.0, 0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0, 0.0]],
                [
                    [0.0, -inf, -inf, -inf, -inf],
                    [0.0, 0.0, -inf, -inf, -inf],
                    [0.0, 0.0, 0.0, -inf, -inf]
                ]
            ]
        );

        let iam = cache.indices_and_mask(3, &[true, true])?;
        let mask = iam.mask.i((.., 0))?.to_vec3::<f32>()?;
        assert_eq!(iam.indices.to_vec2::<u32>()?, [[2, 3, 4], [3, 4, 0]]);
        assert_eq!(
            mask,
            [
                [
                    [0.0, 0.0, 0.0, -inf, -inf],
                    [0.0, 0.0, 0.0, 0.0, -inf],
                    [0.0, 0.0, 0.0, 0.0, 0.0]
                ],
                [
                    [-inf, 0.0, 0.0, 0.0, -inf],
                    [-inf, 0.0, 0.0, 0.0, 0.0],
                    [0.0, 0.0, 0.0, 0.0, 0.0]
                ]
            ]
        );

        let iam = cache.indices_and_mask(1, &[true, false])?;
        let mask = iam.mask.i((.., 0))?.to_vec3::<f32>()?;
        assert_eq!(iam.indices.to_vec2::<u32>()?, [[0], [1]]);
        assert_eq!(mask, [[[0.0, 0.0, 0.0, 0.0, 0.0]], [[0.0, 0.0, 0.0, 0.0, 0.0]]]);

        let iam = cache.indices_and_mask(2, &[true, false])?;
        let mask = iam.mask.i((.., 0))?.to_vec3::<f32>()?;
        assert_eq!(iam.indices.to_vec2::<u32>()?, [[1, 2], [1, 1]]);
        assert_eq!(
            mask,
            [
                [[0.0, 0.0, -inf, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0, 0.0]],
                [[0.0, 0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0, 0.0]]
            ]
        );

        Ok(())
    }
}
