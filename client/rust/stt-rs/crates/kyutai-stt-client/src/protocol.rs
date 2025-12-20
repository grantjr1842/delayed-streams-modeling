use crate::error::{Result, SttError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum InMsg {
    Init,

    Audio { pcm: Vec<f32> },

    OggOpus { data: Vec<u8> },

    Marker { id: i64 },

    Ping,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum OutMsg {
    Word {
        text: String,
        start_time: f64,
    },

    EndWord {
        stop_time: f64,
    },

    Step {
        step_idx: usize,
        prs: Vec<f32>,
        buffered_pcm: usize,
    },

    Marker {
        id: i64,
    },

    Ready,

    Error {
        message: String,
    },
}

pub fn encode_in_msg_into(buf: &mut Vec<u8>, msg: &InMsg) -> Result<()> {
    buf.clear();
    let mut ser = rmp_serde::Serializer::new(buf).with_struct_map();
    msg.serialize(&mut ser)
        .map_err(|e| SttError::Message(e.to_string()))
}

pub fn encode_in_msg(msg: &InMsg) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    encode_in_msg_into(&mut buf, msg)?;
    Ok(buf)
}

pub fn decode_out_msg(bytes: &[u8]) -> Result<OutMsg> {
    rmp_serde::from_slice::<OutMsg>(bytes).map_err(|e| SttError::Message(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_audio_with_vec_f32() {
        let msg = InMsg::Audio {
            pcm: vec![0.0, -0.25, 0.5, 1.0],
        };

        let bytes = encode_in_msg(&msg).expect("encode should succeed");
        let decoded = rmp_serde::from_slice::<InMsg>(&bytes).expect("decode should succeed");

        assert_eq!(msg, decoded);
    }

    #[test]
    fn decode_sample_word_message() {
        let bytes: Vec<u8> = vec![
            0x83, 0xa4, b't', b'y', b'p', b'e', 0xa4, b'W', b'o', b'r', b'd', 0xa4, b't', b'e',
            b'x', b't', 0xa5, b'h', b'e', b'l', b'l', b'o', 0xaa, b's', b't', b'a', b'r', b't',
            b'_', b't', b'i', b'm', b'e', 0xcb, 0x3f, 0xf8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let msg = decode_out_msg(&bytes).expect("decode should succeed");
        assert_eq!(
            msg,
            OutMsg::Word {
                text: "hello".to_string(),
                start_time: 1.5,
            }
        );
    }
}
