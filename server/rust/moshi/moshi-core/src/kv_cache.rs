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
    positions: Vec<usize>,
    // The index where the next element will be stored.
    indices: Vec<usize>,
    dtype: DType,
    device: Device,
}

impl ScatteredCacheBuilder {
    pub fn new(batch_size: usize, context: usize, dtype: DType, device: &Device) -> Result<Self> {
        let positions = vec![0; batch_size];
        let indices = vec![0; batch_size];
        Ok(Self { positions, indices, context, dtype, device: device.clone() })
    }

    pub fn make_cache(&self, num_heads: usize, head_dim: usize) -> Result<ScatteredKvCache> {
        let batch_size = self.batch_size();
        let shape = (batch_size, num_heads, self.context, head_dim);
        let k = Tensor::zeros(shape, self.dtype, &self.device)?;
        let v = Tensor::zeros(shape, self.dtype, &self.device)?;
        Ok(ScatteredKvCache { k, v, context: self.context })
    }

    pub fn positions(&self) -> &[usize] {
        &self.positions
    }

    pub fn reset(&mut self) {
        self.positions.fill(0);
        self.indices.fill(0);
    }

    pub fn batch_size(&self) -> usize {
        self.positions.len()
    }

    pub fn reset_batch_index(&mut self, batch_index: usize) {
        self.positions[batch_index] = 0;
        self.indices[batch_index] = 0;
    }

    #[allow(clippy::needless_range_loop)]
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
            let mut cache_indices = Vec::with_capacity(b);
            let mut attention_masks = Vec::with_capacity(b * context);

            for (batch_i, &active) in batch_mask.iter().enumerate() {
                if !active {
                    cache_indices.push(self.indices[batch_i] as u32);
                    attention_masks.extend(std::iter::repeat(0.0).take(context));
                } else {
                    let index = self.indices[batch_i];
                    let start_pos = self.positions[batch_i];

                    cache_indices.push(index as u32);

                    // Update state
                    self.indices[batch_i] += 1;
                    if self.indices[batch_i] >= context {
                        self.indices[batch_i] = 0;
                    }
                    self.positions[batch_i] += 1;

                    // Generate mask
                    if start_pos < context {
                        let zeros = start_pos + 1;
                        attention_masks.extend(std::iter::repeat(0.0).take(zeros));
                        attention_masks
                            .extend(std::iter::repeat(f32::NEG_INFINITY).take(context - zeros));
                    } else {
                        attention_masks.extend(std::iter::repeat(0.0).take(context));
                    }
                }
            }

            let indices = Tensor::from_vec(cache_indices, (b, 1), &self.device)?;
            let mask = Tensor::from_vec(attention_masks, (b, 1, 1, context), &self.device)?
                .to_dtype(self.dtype)?;
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
        let mut attention_masks = Vec::with_capacity(b * seq_len * context);
        let mut cache_indices = Vec::with_capacity(b * seq_len);
        for (batch_i, &active) in batch_mask.iter().enumerate() {
            if !active {
                attention_masks.extend(std::iter::repeat(0.0).take(seq_len * context));
                cache_indices.extend(std::iter::repeat(self.indices[batch_i] as u32).take(seq_len));
            } else {
                let start_index = self.indices[batch_i];
                let start_pos = self.positions[batch_i];
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
                    let index = self.indices[batch_i];
                    all_pos[index] = seq_i + start_pos;
                    cache_indices.push(index as u32);
                    self.indices[batch_i] += 1;
                    self.positions[batch_i] += 1;
                    if self.indices[batch_i] >= context {
                        self.indices[batch_i] = 0;
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
        for (batch_i, &active) in batch_mask.iter().enumerate() {
            if !active {
                let indices = vec![self.indices[batch_i] as u32; seq_len];
                cache_indices.push(indices);
            } else {
                let mut indices = Vec::with_capacity(seq_len);
                for _ in 0..seq_len {
                    let index = self.indices[batch_i];
                    indices.push(index as u32);
                    self.indices[batch_i] += 1;
                    self.positions[batch_i] += 1;
                    if self.indices[batch_i] >= self.context {
                        self.indices[batch_i] = 0;
                    }
                }
                cache_indices.push(indices);
            }
        }
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
