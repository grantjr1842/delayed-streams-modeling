use crate::types::WordTiming;

#[derive(Clone, Debug, Default)]
pub struct TranscriptAssembler {
    pending_word: Option<PendingWord>,
}

#[derive(Clone, Debug)]
struct PendingWord {
    text: String,
    start_time_s: f64,
}

impl TranscriptAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_word(&mut self, text: String, start_time: f64) -> Option<WordTiming> {
        let flushed = self.pending_word.take().map(|pending| WordTiming {
            word: pending.text,
            start_ms: sec_to_ms(pending.start_time_s),
            end_ms: sec_to_ms(pending.start_time_s),
            confidence: None,
        });

        self.pending_word = Some(PendingWord {
            text,
            start_time_s: start_time,
        });

        flushed
    }

    pub fn push_end_word(&mut self, stop_time: f64) -> Option<WordTiming> {
        self.finalize_pending(stop_time)
    }

    fn finalize_pending(&mut self, stop_time_s: f64) -> Option<WordTiming> {
        let pending = self.pending_word.take()?;

        let start_ms = sec_to_ms(pending.start_time_s);
        let end_ms = sec_to_ms(stop_time_s).max(start_ms);

        Some(WordTiming {
            word: pending.text,
            start_ms,
            end_ms,
            confidence: None,
        })
    }
}

fn sec_to_ms(s: f64) -> u64 {
    if !s.is_finite() || s.is_sign_negative() {
        return 0;
    }

    (s * 1000.0).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_then_endword_yields_word_timing() {
        let mut a = TranscriptAssembler::new();

        assert!(
            a.push_word("hello".to_string(), 1.0).is_none()
        );

        let w = a
            .push_end_word(1.25)
            .expect("should finalize pending word");

        assert_eq!(w.word, "hello");
        assert_eq!(w.start_ms, 1000);
        assert_eq!(w.end_ms, 1250);
        assert!(w.confidence.is_none());
    }

    #[test]
    fn endword_without_word_is_ignored() {
        let mut a = TranscriptAssembler::new();
        assert!(a.push_end_word(1.0).is_none());
    }

    #[test]
    fn word_overwrites_pending_flushing_best_effort_timing() {
        let mut a = TranscriptAssembler::new();

        assert!(a.push_word("a".to_string(), 0.1).is_none());

        let flushed = a
            .push_word("b".to_string(), 0.2)
            .expect("previous pending word should be flushed");

        assert_eq!(flushed.word, "a");
        assert_eq!(flushed.start_ms, 100);
        assert_eq!(flushed.end_ms, 100);

        let w = a
            .push_end_word(0.3)
            .expect("second word should be finalized by endword");

        assert_eq!(w.word, "b");
        assert_eq!(w.start_ms, 200);
        assert_eq!(w.end_ms, 300);
    }
}
