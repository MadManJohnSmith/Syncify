/* Lossless token editing for enhanced LRC and textual KRC. Unedited spans are kept verbatim. */
export interface LyricToken {
    text: string;
    timestamp: string;
    /** Original marker and text, retained when the token is unchanged. */
    originalText: string;
    originalTimestamp: string;
    marker: string;
    kind: 'lrc' | 'krc';
}
export interface TokenLine {
    prefix: string;
    tokens: LyricToken[];
    ending: string;
}
export interface TokenDocument { lines: TokenLine[]; original: string }

const LRC_MARKER = /<\d{1,3}:\d{2}(?:[.:]\d{1,3})?>/g;
const KRC_MARKER = /<\d+,\d+,\d+>/g;
const LRC_LINE = /^(?:\[\d{1,3}:\d{2}(?:[.:]\d{1,3})?\])+[\t ]*/;
const KRC_LINE = /^\[\d+,\d+\][\t ]*/;

export function parseTokenDocument(content: string, level: 'word' | 'syllable'): TokenDocument {
    const lines = content.match(/[^\r\n]*(?:\r\n|\n|\r|$)/g)?.filter(Boolean) ?? [];
    return { original: content, lines: lines.map(raw => {
        const ending = raw.match(/\r\n|\n|\r$/)?.[0] ?? '';
        const body = raw.slice(0, raw.length - ending.length);
        const prefixMatch = body.match(LRC_LINE) ?? body.match(KRC_LINE);
        if (!prefixMatch) return { prefix: body, tokens: [], ending };
        const kind: LyricToken['kind'] = KRC_LINE.test(prefixMatch[0]) ? 'krc' : 'lrc';
        const markerPattern = kind === 'krc' ? KRC_MARKER : LRC_MARKER;
        const rest = body.slice(prefixMatch[0].length);
        const matches = [...rest.matchAll(markerPattern)];
        if (!matches.length) return { prefix: body, tokens: [], ending };
        const prefix = prefixMatch[0] + rest.slice(0, matches[0].index);
        const tokens = matches.map((match, index) => {
            const marker = match[0];
            const text = rest.slice(match.index + marker.length, matches[index + 1]?.index ?? rest.length);
            const timestamp = marker.slice(1, -1);
            return { text, timestamp, originalText: text, originalTimestamp: timestamp, marker, kind };
        });
        return { prefix, tokens, ending };
    }) };
}

export function validateTokenDocument(doc: TokenDocument): string | null {
    if (!doc.lines.some(line => line.tokens.length)) return 'No timed tokens found.';
    for (const [lineIndex, line] of doc.lines.entries()) {
        for (const [tokenIndex, token] of line.tokens.entries()) {
            const valid = token.kind === 'lrc'
                ? /^(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?$/.exec(token.timestamp)
                : /^(\d+),(\d+),(\d+)$/.exec(token.timestamp);
            if (!valid || (token.kind === 'lrc' && Number(valid[2]) >= 60))
                return `Token ${tokenIndex + 1}, line ${lineIndex + 1}: invalid timestamp.`;
        }
    }
    return null;
}

export function serializeTokenDocument(doc: TokenDocument): string {
    const error = validateTokenDocument(doc);
    if (error) throw new Error(error);
    return doc.lines.map(line => line.prefix + line.tokens.map(token => {
        const marker = token.timestamp === token.originalTimestamp ? token.marker : `<${token.timestamp}>`;
        return marker + token.text;
    }).join('') + line.ending).join('');
}

export function tokenDocumentChanged(doc: TokenDocument): boolean {
    return doc.lines.some(line => line.tokens.some(token => token.text !== token.originalText || token.timestamp !== token.originalTimestamp));
}
