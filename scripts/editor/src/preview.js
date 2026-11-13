// What draws a Markdown file as a document to read: the lab's own renderer on a page with no
// framework, its stylesheet (the page puts it in, under its own tokens), raw HTML made safe, and
// the colours of fenced code.
import {mount} from "@localix-ai/flowdown/dom";
import {html} from "@localix-ai/flowdown/html";
import {highlight} from "@localix-ai/flowdown/highlight";
import sheet from "@localix-ai/flowdown/styles.css" with {type: "text"};

CM.preview = {mount, html, highlight, sheet};
