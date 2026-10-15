import { describe, expect, it } from 'vitest';
import { parseTokenDocument, serializeTokenDocument, tokenDocumentChanged, validateTokenDocument } from '@/api/lyricTokens';

describe('lossless word/syllable lyric tokens', () => {
    it('roundtrips Unicode enhanced LRC with metadata, multiple line marks and CRLF', () => {
        const source = '[ti:漢字 canción]\r\n[00:01.00][00:02.00] <00:01.00>你好 🌟 <00:01.50>café\r\n';
        const doc = parseTokenDocument(source, 'word');
        expect(doc.lines[1].tokens.map(t => [t.text, t.timestamp])).toEqual([
            ['你好 🌟 ', '00:01.00'], ['café', '00:01.50'],
        ]);
        expect(validateTokenDocument(doc)).toBeNull();
        expect(serializeTokenDocument(doc)).toBe(source);
        doc.lines[1].tokens[1].text = 'mañana';
        expect(tokenDocumentChanged(doc)).toBe(true);
        expect(serializeTokenDocument(doc)).toBe(source.replace('café', 'mañana'));
    });

    it('roundtrips textual KRC syllables and changes only selected token', () => {
        const source = '[offset:0]\n[1000,2400]<0,500,0>你<500,700,0>好 🌍<1200,500,0>!\n';
        const doc = parseTokenDocument(source, 'syllable');
        expect(doc.lines[1].tokens).toHaveLength(3);
        expect(serializeTokenDocument(doc)).toBe(source);
        doc.lines[1].tokens[1].timestamp = '550,700,0';
        expect(serializeTokenDocument(doc)).toBe(source.replace('<500,700,0>', '<550,700,0>'));
        doc.lines[1].tokens[0].timestamp = 'oops';
        expect(validateTokenDocument(doc)).toMatch(/invalid timestamp/);
        expect(() => serializeTokenDocument(doc)).toThrow(/invalid timestamp/);
    });
});
