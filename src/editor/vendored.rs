//! The editor's vendored scripts and their SHA-384s: the core every page runs, then a script a
//! language. Written by `scripts/editor/build.ts`, never by hand; a page runs each by its hash,
//! so a script that was altered never runs.

/// A vendored script: its name, its text, and the SHA-384 a page's policy runs it by.
pub(super) struct Script {
    pub(super) name: &'static str,
    pub(super) text: &'static str,
    pub(super) sha384: &'static str,
}

pub(super) const SCRIPTS: &[Script] = &[
    Script {
        name: "core",
        text: include_str!("vendor/core.js"),
        sha384: "KDl09XPMoxnTA7PRNqfPr5/vTlrpx2IJ4Tf3xSF1oM+oM2fV0SdxmBUhGZzfkYFF",
    },
    Script {
        name: "cpp",
        text: include_str!("vendor/cpp.js"),
        sha384: "lfZlIgc1FiAJ+ZZjZZR7w8t4NzyX3T9wvESf3scm27Nwrgu71Q70r+SePvg3hVA2",
    },
    Script {
        name: "css",
        text: include_str!("vendor/css.js"),
        sha384: "co4TMs/XSym1cyqcNDUoDi7r/Tg/df/SdIbuhDmn+XtIUrKA/hgiGsLw1xL9/Fiz",
    },
    Script {
        name: "go",
        text: include_str!("vendor/go.js"),
        sha384: "mKNyiuPku4VnrsFzGSoFMVwZIxbVOjBhugeryPhfZBKbvjl8IdAKlViZxw/zDPJg",
    },
    Script {
        name: "html",
        text: include_str!("vendor/html.js"),
        sha384: "T0k6fcNNX7mR4eQCV/a8/d1aGd3mCQUAHz6W/Vq3GG7jHwSWJs4fJaeNHN0Kx8bw",
    },
    Script {
        name: "java",
        text: include_str!("vendor/java.js"),
        sha384: "3BIJ9EQ2JEJYKiugkEXO7jjJDR3XqJRlWWsd1bFIlfLn1SgKbod7nLDoR2zJZXvu",
    },
    Script {
        name: "javascript",
        text: include_str!("vendor/javascript.js"),
        sha384: "CRxm91NKnBwRC0br5J7oF00cFJSIoZ++Epm91JBb2r2nP/KqtqF9jxxl5j8tdzvr",
    },
    Script {
        name: "json",
        text: include_str!("vendor/json.js"),
        sha384: "cqdRLg6VnoRTYyimYtWJ/350s4iYchjasJN2JlDjNhoE88S25Z1zhHecfZKKQsLp",
    },
    Script {
        name: "markdown",
        text: include_str!("vendor/markdown.js"),
        sha384: "Q/Rw/cbG+h/2+4f9e+nbPd1kwvMOkpFMoqxw3Wk0J6oCkSrr9dUMNBctN4eaHrYl",
    },
    Script {
        name: "modes",
        text: include_str!("vendor/modes.js"),
        sha384: "rsCeF9YtXmZk7pRZCFEtKc9tljO7Z87+VzRgsW2TwI2RplX8x6BOF1mvY0gH1yDH",
    },
    Script {
        name: "php",
        text: include_str!("vendor/php.js"),
        sha384: "D3BwlmEwts62c8YF2qKtKts1BuOQ8A4tg95eWr/qv6LcD7fktl4hjcG1Sl+bXHz/",
    },
    Script {
        name: "python",
        text: include_str!("vendor/python.js"),
        sha384: "hIBJqpEoauqkM6UcWshH1duTqCgnLLdjGY4Yy5kkgL9oul/2mbuDjP1jJzDKrzcw",
    },
    Script {
        name: "rust",
        text: include_str!("vendor/rust.js"),
        sha384: "dzhAo85GzGxIeKO5yMTGYppCssjr1Kt0CjNmqhkCOMn9sfsWREmamk7GzdHEr8uk",
    },
    Script {
        name: "sql",
        text: include_str!("vendor/sql.js"),
        sha384: "semSqQr4AAVDOH4mBpyakCDxaLThdmjymJcrWLo804DzsCmfeeBC0vABIY0pHmde",
    },
    Script {
        name: "xml",
        text: include_str!("vendor/xml.js"),
        sha384: "JqeB9lKJv0gnfnDIsj/9BCwiKzuLJCZrs5kncTHg9R5SVjXMs1Bn76BAagsaT6l7",
    },
    Script {
        name: "yaml",
        text: include_str!("vendor/yaml.js"),
        sha384: "fAm3WDLS0y27doqNOqfUN93P+6CvF9Gdw/4RIUjQQ2k3Zitj8qD7XiL4tUShe8vT",
    },
];
