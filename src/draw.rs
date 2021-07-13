pub struct Characters {
    pub hbar: char,
    pub vbar: char,
    pub topl: char,
    pub topr: char,
    pub botl: char,
    pub botr: char,
}

impl Characters {
    pub fn unicode() -> Self {
        Self {
            hbar: '─',
            vbar: '│',
            topl: '╭',
            topr: '╮',
            botl: '╰',
            botr: '╯',
        }
    }
}
