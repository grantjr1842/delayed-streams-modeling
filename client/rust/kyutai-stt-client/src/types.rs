#[derive(Clone, Debug)]
pub struct WordTiming {
    pub word: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub confidence: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct Utterance {
    pub text: String,
}

#[derive(Clone, Debug)]
pub enum SttEvent {
    Ready,
    WordReceived {
        text: String,
        start_ms: u64,
    },
    WordFinalized(WordTiming),
    UtterancePartial(Utterance),
    UtteranceFinal(Utterance),
    VadStep {
        step_idx: usize,
        prs: Vec<f32>,
        buffered_pcm: usize,
    },
    StreamMarker {
        id: i64,
    },
    Error {
        message: String,
    },
}
