pub struct Characters {
    pub hbar: char,
    pub vbar: char,
    pub vbar_break: char,

    pub ltop: char,
    pub rtop: char,
    pub lbot: char,
    pub rbot: char,

    pub lbox: char,
    pub rbox: char,

    pub lcross: char,
    pub rcross: char,
}

impl Characters {
    pub fn unicode() -> Self {
        Self {
            hbar: '─',
            vbar: '│',
            vbar_break: '·',
            ltop: '╭',
            rtop: '╮',
            lbot: '╰',
            rbot: '╯',
            lbox: '[',
            rbox: ']',
            lcross: '├',
            rcross: '┤',
        }
    }

    pub fn ascii() -> Self {
        Self {
            hbar: '-',
            vbar: '|',
            vbar_break: ':',
            ltop: ',',
            rtop: '.',
            lbot: '`',
            rbot: '\'',
            lbox: '[',
            rbox: ']',
            lcross: '|',
            rcross: '|',
        }
    }

    pub fn extended_ascii() -> Self {
        Self {
            hbar: '─',
            vbar: '│',
            vbar_break: '·',
            ltop: '┌',
            rtop: '┐',
            lbot: '└',
            rbot: '┘',
            lbox: '[',
            rbox: ']',
            lcross: '├',
            rcross: '┤',
        }
    }
}
