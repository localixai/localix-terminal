# Third-party brand marks

The Claude, OpenAI/Codex, Gemini, GitHub Copilot and OpenCode marks in `src/browser/marks/` (the
coding agents "+" starts) are the SVG paths of the MIT-licensed
[CodexBar](https://github.com/steipete/CodexBar) brand mark set. The marks identify their
respective products; ownership of the brands remains with their owners. The Hermes mark is drawn
here.

MIT License

Copyright (c) 2026 Peter Steinberger

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial
portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## Hermes

The Hermes silhouette is derived from `assets/icon-master.svg` in the
[Nous Research Hermes Agent repository](https://github.com/NousResearch/hermes-agent). The app-icon
background is omitted; the path inherits the surrounding text colour.

MIT License

Copyright (c) 2025 Nous Research

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial
portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## File pictures

The pictures of files and folders in `src/browser/file_icons/` are from the MIT-licensed
[vscode-icons](https://github.com/vscode-icons/vscode-icons) set. Where a picture is a
product's mark, it identifies that product; ownership of the mark remains with its owner.

Copyright (c) 2016 Roberto Huertas

Licensed under the MIT License, whose text is above.

## The editor

The scripts in `src/editor/vendor/` are built by `scripts/editor` from the MIT-licensed
[CodeMirror](https://codemirror.net) packages (`@codemirror/*`) and the
[Lezer](https://lezer.codemirror.net) parsers (`@lezer/*`), at the versions `scripts/editor/package.json` pins.

Copyright (C) 2018-2021 by Marijn Haverbeke <marijn@haverbeke.berlin> and others

Licensed under the MIT License, whose text is above.

## The terminal core

The terminal core (`src/terminal/core.rs`) is [libghostty-vt](https://ghostty.org) 1.0.1, the
terminal emulation library of Ghostty (MIT), linked statically into the program through the
`ghostty-vt-sys` crate's bindings (Apache-2.0). Built into the library are
[simdutf](https://github.com/simdutf/simdutf) 9.0.0 (MIT or Apache-2.0, taken under MIT),
[Highway](https://github.com/google/highway) 1.2.0 (Apache-2.0 or BSD-3-Clause, taken under
BSD-3-Clause), [Wuffs](https://github.com/google/wuffs) 0.4.0-alpha.10 (MIT or Apache-2.0, taken
under MIT), the Unicode tables of uucode 0.2.0 (MIT), and the runtime routines the Zig compiler
builds into a library, its compiler_rt (MIT).

Copyright (c) 2024 Mitchell Hashimoto, Ghostty contributors

Copyright 2021 The simdutf authors

Copyright 2023 The Wuffs Authors

Copyright (c) 2026 Jacob Sandlund

Copyright (c) Zig contributors

Each licensed under the MIT License, whose text is above.

Copyright (c) The Highway Project Authors. All rights reserved.

Redistribution and use in source and binary forms, with or without modification, are permitted
provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions
   and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice, this list of
   conditions and the following disclaimer in the documentation and/or other materials provided
   with the distribution.
3. Neither the name of the copyright holder nor the names of its contributors may be used to
   endorse or promote products derived from this software without specific prior written
   permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR
IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR
CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER
IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT
OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
