// The editor's core: the packages every language shares, whole, and the few commands the page
// binds. A language's script takes the shared packages from here, so one copy of each runs.
import * as state from "@codemirror/state";
import * as view from "@codemirror/view";
import * as language from "@codemirror/language";
import * as common from "@lezer/common";
import * as lr from "@lezer/lr";
import * as highlight from "@lezer/highlight";
import {defaultKeymap, history, historyKeymap, indentWithTab, undoDepth, redoDepth} from "@codemirror/commands";
import {search, searchKeymap, highlightSelectionMatches, openSearchPanel, gotoLine} from "@codemirror/search";
import {closeBrackets, closeBracketsKeymap} from "@codemirror/autocomplete";

globalThis.CM = {
  pkg: {
    "@codemirror/state": state,
    "@codemirror/view": view,
    "@codemirror/language": language,
    "@lezer/common": common,
    "@lezer/lr": lr,
    "@lezer/highlight": highlight,
  },
  commands: {defaultKeymap, history, historyKeymap, indentWithTab, undoDepth, redoDepth},
  search: {search, searchKeymap, highlightSelectionMatches, openSearchPanel, gotoLine},
  close: {closeBrackets, closeBracketsKeymap},
  langs: {},
};
