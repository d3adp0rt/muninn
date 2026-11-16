//! Server-sent events parser.
//!
//! OpenRouter uses SSE with a keep-alive comment (`: OPENROUTER PROCESSING`)
//! that must be ignored, and `data: [DONE]` as the terminator.
//!
//! The parser is deliberately independent of the transport so it can be tested
//! with arbitrary chunk boundaries (including splits inside a UTF-8 sequence).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
    data: String,
    event: Option<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed raw bytes. Returns all complete events.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();

        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let mut line: Vec<u8> = self.buf.drain(..=pos).collect();
            line.pop(); // '\n'
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = String::from_utf8_lossy(&line);

            if line.is_empty() {
                self.dispatch(&mut out);
                continue;
            }

            self.handle_line(&line);
        }

        out
    }

    /// Flush a final event if the stream ended without a trailing blank line.
    pub fn finish(&mut self) -> Vec<SseEvent> {
        let mut out = Vec::new();

        if !self.buf.is_empty() {
            let mut line = std::mem::take(&mut self.buf);
            if line.last() == Some(&b'\n') {
                line.pop();
            }
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = String::from_utf8_lossy(&line).into_owned();
            if !line.is_empty() {
                self.handle_line(&line);
            }
        }

        self.dispatch(&mut out);
        out
    }

    fn handle_line(&mut self, line: &str) {
        if line.starts_with(':') {
            // Comment / keep-alive.
            return;
        }

        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);

        match field {
            "data" => {
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(value);
            }
            "event" => self.event = Some(value.to_owned()),
            _ => {}
        }
    }

    fn dispatch(&mut self, out: &mut Vec<SseEvent>) {
        if self.data.is_empty() && self.event.is_none() {
            return;
        }
        let mut data = std::mem::take(&mut self.data);
        if data.ends_with('\n') {
            data.pop();
        }
        out.push(SseEvent {
            event: self.event.take(),
            data,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(chunks: &[&str]) -> Vec<SseEvent> {
        let mut p = SseParser::new();
        let mut out = Vec::new();
        for c in chunks {
            out.extend(p.push(c.as_bytes()));
        }
        out.extend(p.finish());
        out
    }

    #[test]
    fn simple_data() {
        let events = parse(&["data: {\"a\":1}\n\n"]);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "{\"a\":1}");
    }

    #[test]
    fn ignore_keepalive_and_comments() {
        let events = parse(&[": OPENROUTER PROCESSING\n\ndata: hello\n\n"]);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn split_across_chunks() {
        let events = parse(&["da", "ta: hel", "lo\n", "\n"]);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn crlf_and_multiline_data() {
        let events = parse(&["event: x\r\ndata: a\r\ndata: b\r\n\r\n"]);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.as_deref(), Some("x"));
        assert_eq!(events[0].data, "a\nb");
    }

    #[test]
    fn finish_without_blank_line() {
        let events = parse(&["data: [DONE]"]);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "[DONE]");
    }

    #[test]
    fn utf8_split_inside_codepoint() {
        let text = "data: Привет\n\n";
        let bytes = text.as_bytes();
        let (a, b) = bytes.split_at(9); // inside a Cyrillic code point
        let mut p = SseParser::new();
        let mut events = p.push(a);
        events.extend(p.push(b));
        events.extend(p.finish());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "Привет");
    }
}
