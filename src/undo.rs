use serde::{Serialize, de::DeserializeOwned};
pub struct Undo<T> {
    frames: Vec<T>,
    sizes: Vec<usize>,
    index: usize,
    limit: usize,
    budget: usize,
    total: usize,
    last_key: Option<String>,
    last_time: u64,
    encoded: String,
}
fn encode<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("Undo state must be serializable")
}
impl<T: Clone + Serialize + DeserializeOwned> Undo<T> {
    pub fn new(initial: T, limit: usize, budget: usize) -> Self {
        let encoded = encode(&initial);
        let size = encoded.encode_utf16().count() * 4;
        Self {
            frames: vec![initial],
            sizes: vec![size],
            index: 0,
            limit,
            budget,
            total: size,
            last_key: None,
            last_time: 0,
            encoded,
        }
    }
    pub fn record(&mut self, value: T, key: Option<&str>, now: u64) -> bool {
        let encoded = encode(&value);
        if encoded == self.encoded {
            return false;
        }
        let merge = self.index > 0
            && self.index == self.frames.len() - 1
            && key.is_some()
            && key == self.last_key.as_deref()
            && now.saturating_sub(self.last_time) < 700;
        self.frames.truncate(self.index + 1);
        self.sizes.truncate(self.index + 1);
        self.total = self.sizes.iter().sum();
        let size = encoded.encode_utf16().count() * 4;
        if merge {
            self.total -= self.sizes[self.index];
            self.frames[self.index] = value;
            self.sizes[self.index] = size;
        } else {
            self.frames.push(value);
            self.sizes.push(size);
            self.index += 1;
        }
        self.total += size;
        while self.frames.len() > 1 && (self.frames.len() > self.limit || self.total > self.budget)
        {
            self.frames.remove(0);
            self.total -= self.sizes.remove(0);
            self.index -= 1;
        }
        self.encoded = encoded;
        self.last_key = key.map(str::to_string);
        self.last_time = now;
        true
    }
    pub fn undo(&mut self) -> Option<T> {
        if !self.can_undo() {
            return None;
        }
        self.last_key = None;
        self.index -= 1;
        self.encoded = encode(&self.frames[self.index]);
        Some(self.frames[self.index].clone())
    }
    pub fn redo(&mut self) -> Option<T> {
        if !self.can_redo() {
            return None;
        }
        self.last_key = None;
        self.index += 1;
        self.encoded = encode(&self.frames[self.index]);
        Some(self.frames[self.index].clone())
    }
    pub fn can_undo(&self) -> bool {
        self.index > 0
    }
    pub fn can_redo(&self) -> bool {
        self.index + 1 < self.frames.len()
    }
}
