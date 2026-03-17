# tools/patches/patch_alias_fnname.py
import re

ALIAS_SNIPPET = (
    '\n                    // Also register a short alias if the parser produced a dotted name.\n'
    '                    if let Some(pos) = name.name.rfind(\'.\') {\n'
    '                        let short = name.name[pos+1..].to_string();\n'
    '                        if !self.functions.contains_key(&short) {\n'
    '                            self.functions.insert(short.clone(), (params.clone(), body.clone()));\n'
    '                        }\n'
    '                        if params.is_empty() {\n'
    '                            self.env.set_global(short, Value::Lambda(body.clone()));\n'
    '                        }\n'
    '                    } else {\n'
    '                        if params.is_empty() {\n'
    '                            self.env.set_global(name.name.clone(), Value::Lambda(body.clone()));\n'
    '                        }\n'
    '                    }\n'
)

def apply(text: str) -> str:
    anchor = r"self\.functions\.insert\(\s*name\.name\.clone\(\)\s*,\s*\(params\.clone\(\)\s*,\s*body\.clone\(\)\)\s*\)\s*;"
    # find first occurrence and insert snippet after the semicolon
    m = re.search(anchor, text)
    if not m:
        raise ValueError("anchor not found for alias insertion")
    insert_pos = m.end()
    new_text = text[:insert_pos] + ALIAS_SNIPPET + text[insert_pos:]
    return new_text
