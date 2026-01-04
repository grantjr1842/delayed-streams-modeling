use anyhow::Result;
use candle::{Device, Tensor};
use moshi::asr::{AsrModel, State};
use moshi::conditioner::Condition;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asr_model_creation() -> Result<()> {
        // Test ASR model initialization
        let device = Device::Cpu;
        let model = AsrModel::new(&device)?;
        assert!(model.is_ok());
        Ok(())
    }

    #[test]
    fn test_streaming_state() -> Result<()> {
        // Test streaming state management
        let device = Device::Cpu;
        let mut state = State::new(1, &device)?;

        // Test initial state
        assert_eq!(state.batch_size(), 1);

        // Test state reset
        state.reset_state();
        assert_eq!(state.batch_size(), 1);

        Ok(())
    }

    #[test]
    fn test_conditioner() -> Result<()> {
        // Test audio conditioning
        let device = Device::Cpu;
        let conditioner = Condition::new(0.5, &device)?;

        let condition = conditioner.condition(0.8)?;
        assert_eq!(condition.value(), 0.4); // 0.8 * 0.5

        Ok(())
    }

    #[test]
    fn test_tensor_operations() -> Result<()> {
        // Test tensor operations used in ASR
        let device = Device::Cpu;

        // Test audio tensor creation
        let audio_data = vec![0.1f32; 1600]; // 100ms at 16kHz
        let audio_tensor = Tensor::from_vec(audio_data, (1, 1, 1600), &device)?;
        assert_eq!(audio_tensor.dims(), &[1, 1, 1600]);

        // Test batch processing
        let batched_audio = audio_tensor.repeat(0, 4)?; // Create batch of 4
        assert_eq!(batched_audio.dims(), &[4, 1, 1600]);

        Ok(())
    }

    #[test]
    fn test_error_handling() {
        // Test error scenarios
        let device = Device::Cpu;

        // Test invalid tensor dimensions
        let invalid_data = vec![0.1f32; 10];
        let result = Tensor::from_vec(invalid_data, (1, 1, 1600), &device);
        assert!(result.is_err());

        // Test invalid batch size
        let state_result = State::new(0, &device);
        assert!(state_result.is_err());
    }
}
