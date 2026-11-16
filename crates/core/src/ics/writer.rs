const FOLD_AT: usize = 75;

#[derive(Default)]
pub(super) struct Writer {
    out: String,
}

impl Writer {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn finish(self) -> String {
        self.out
    }

    pub(super) fn begin(&mut self, component: impl AsRef<str>) {
        self.property("BEGIN", component);
    }

    pub(super) fn end(&mut self, component: impl AsRef<str>) {
        self.property("END", component);
    }

    pub(super) fn property(&mut self, name: impl AsRef<str>, value: impl AsRef<str>) {
        self.line(format!("{}:{}", name.as_ref(), value.as_ref()));
    }

    pub(super) fn line(&mut self, line: impl AsRef<str>) {
        let line = line.as_ref();
        let mut current = String::new();
        let mut current_len = 0usize;

        for ch in line.chars() {
            let ch_len = ch.len_utf8();
            if current_len + ch_len > FOLD_AT {
                // Do not split an escape sequence across a folded line.
                let trailing_backslashes = current
                    .chars()
                    .rev()
                    .take_while(|&character| character == '\\')
                    .count();
                if trailing_backslashes % 2 == 1 {
                    current.pop();
                    self.out.push_str(&current);
                    self.out.push_str("\r\n ");
                    current.clear();
                    current.push('\\');
                    current_len = 2;
                } else {
                    self.out.push_str(&current);
                    self.out.push_str("\r\n ");
                    current.clear();
                    current_len = 1;
                }
            }

            current.push(ch);
            current_len += ch_len;
        }

        self.out.push_str(&current);
        self.out.push_str("\r\n");
    }
}
