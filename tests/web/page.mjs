// The dashboard as the browser assembles it: the shell with the stylesheet and every script put back
// where their tags are, in the order the tags load them. The tests lift marked blocks and read markup
// from this text, so a block can sit in any of the web/*.js files and a test need not know which one.
import { readFileSync } from 'node:fs';

const web = (path) => readFileSync(new URL('../../web/' + path, import.meta.url), 'utf8');

export const shell = web('index.html');
export const css = web('app.css');
/** File names under web/js order, as the shell's script tags list them. */
export const scriptFiles = [...shell.matchAll(/<script src="\/js\/([^"]+)"><\/script>/g)].map((m) => m[1]);
export const scriptTexts = scriptFiles.map((f) => web(f));
export const script = scriptTexts.join('');
export const html = shell
  .replace('<link rel="stylesheet" href="/app.css">', () => '<style>\n' + css + '</style>')
  .replace(/(<script src="\/js\/[^"]+"><\/script>\n)+/, () => '<script>\n' + script + '</script>\n');
export const webFile = web;
