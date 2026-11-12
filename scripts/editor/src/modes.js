// The languages with no grammar of their own here: each a small line-by-line mode.
import {StreamLanguage} from "@codemirror/language";
import {shell} from "@codemirror/legacy-modes/mode/shell";
import {toml} from "@codemirror/legacy-modes/mode/toml";
import {dockerFile} from "@codemirror/legacy-modes/mode/dockerfile";
import {ruby} from "@codemirror/legacy-modes/mode/ruby";
import {swift} from "@codemirror/legacy-modes/mode/swift";
import {lua} from "@codemirror/legacy-modes/mode/lua";
import {diff} from "@codemirror/legacy-modes/mode/diff";
import {properties} from "@codemirror/legacy-modes/mode/properties";
import {kotlin, csharp, scala, dart} from "@codemirror/legacy-modes/mode/clike";

const modes = {shell, toml, dockerfile: dockerFile, ruby, swift, lua, diff, properties, kotlin, csharp, scala, dart};
for (const name in modes) CM.langs[name] = StreamLanguage.define(modes[name]);
