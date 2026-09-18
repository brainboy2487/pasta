/* jsmn.c - Minimal JSMN implementation (public domain subset) */
#include <string.h>
#include "jsmn.h"

static jsmntok_t *jsmn_alloc_token(jsmn_parser *parser, jsmntok_t *tokens, unsigned int num_tokens) {
    if (parser->toknext >= num_tokens) return NULL;
    jsmntok_t *tok = &tokens[parser->toknext++];
    tok->start = tok->end = -1;
    tok->size = 0;
    tok->type = JSMN_UNDEFINED;
    return tok;
}

void jsmn_init(jsmn_parser *parser) {
    parser->pos = 0;
    parser->toknext = 0;
    parser->toksuper = -1;
}

/* Very small JSON parser: supports objects, arrays, strings, primitives. */
int jsmn_parse(jsmn_parser *parser, const char *js, size_t len,
               jsmntok_t *tokens, unsigned int num_tokens) {
    int r = 0;
    for (; parser->pos < len; parser->pos++) {
        char c = js[parser->pos];
        switch (c) {
            case '{':
            case '[': {
                jsmntok_t *tok = jsmn_alloc_token(parser, tokens, num_tokens);
                if (!tok) return JSMN_ERROR_NOMEM;
                tok->type = (c == '{') ? JSMN_OBJECT : JSMN_ARRAY;
                tok->start = parser->pos;
                tok->size = 0;
                parser->toksuper = parser->toknext - 1;
                break;
            }
            case '}':
            case ']': {
                jsmntype_t type = (c == '}') ? JSMN_OBJECT : JSMN_ARRAY;
                int i = parser->toknext - 1;
                for (; i >= 0; --i) {
                    jsmntok_t *tok = &tokens[i];
                    if (tok->start != -1 && tok->end == -1) {
                        if (tok->type != type) return JSMN_ERROR_INVAL;
                        tok->end = parser->pos + 1;
                        parser->toksuper = -1;
                        break;
                    }
                }
                break;
            }
            case '"': {
                jsmntok_t *tok = jsmn_alloc_token(parser, tokens, num_tokens);
                if (!tok) return JSMN_ERROR_NOMEM;
                tok->type = JSMN_STRING;
                tok->start = parser->pos + 1;
                parser->pos++;
                while (parser->pos < len) {
                    char ch = js[parser->pos];
                    if (ch == '"') { tok->end = parser->pos; break; }
                    if (ch == '\\' && parser->pos + 1 < len) parser->pos += 1;
                    parser->pos++;
                }
                if (tok->end == -1) return JSMN_ERROR_PART;
                break;
            }
            case '\t': case '\r': case '\n': case ' ': case ',': case ':':
                break;
            default: {
                /* primitives: number, true, false, null */
                jsmntok_t *tok = jsmn_alloc_token(parser, tokens, num_tokens);
                if (!tok) return JSMN_ERROR_NOMEM;
                tok->type = JSMN_PRIMITIVE;
                tok->start = parser->pos;
                while (parser->pos < len) {
                    char ch = js[parser->pos];
                    if (ch == ',' || ch == ']' || ch == '}' || ch == '\t' || ch == '\r' || ch == '\n' || ch == ' ') {
                        tok->end = parser->pos;
                        break;
                    }
                    parser->pos++;
                }
                if (tok->end == -1) tok->end = parser->pos;
                break;
            }
        }
    }
    return parser->toknext;
}
