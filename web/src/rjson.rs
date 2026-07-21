//! RJson — RGit WASMフロントエンド専用の最小JSONパーサ。
//!
//! `js_sys::JSON::parse`(ブラウザ組み込み)を使うと、パース結果は
//! `JsValue`(JS側オブジェクト)のまま残り、フィールドを読むたびに
//! `Reflect::get`でWASM↔JS境界を1回ずつ跨ぐ。ここで扱うJSONは
//! `/api/repos`(文字列配列)・`/api/repos/:name/readme`
//! (`{branch, content}`)の2種類のみで構造が単純なため、素のRustで
//! パースして`String`/`Vec`としてWASM線形メモリ内に閉じ込めた方が、
//! 境界越えの呼び出し回数を削減できる(パース後のアクセスはすべて
//! ネイティブRust値への操作になる)。
//!
//! JSON仕様のうち、このプロジェクトが受け取るレスポンス(`serde_json`が
//! サーバー側で生成する標準的なJSON)を正しく読める範囲は網羅する
//! (文字列エスケープ`\"` `\\` `\/` `\n` `\t` `\r` `\b` `\f` `\uXXXX`、
//! 数値、真偽値、null、配列、オブジェクト)。汎用JSONライブラリの代替を
//! 名乗るものではなく、あくまでこのクレート内で完結する最小実装。

use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

pub fn parse(input: &str) -> Result<Value, String> {
    let mut chars = input.chars().peekable();
    skip_whitespace(&mut chars);
    let value = parse_value(&mut chars)?;
    skip_whitespace(&mut chars);
    Ok(value)
}

fn skip_whitespace(chars: &mut Peekable<Chars>) {
    while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
        chars.next();
    }
}

fn parse_value(chars: &mut Peekable<Chars>) -> Result<Value, String> {
    skip_whitespace(chars);
    match chars.peek() {
        Some('"') => parse_string(chars).map(Value::Str),
        Some('{') => parse_object(chars),
        Some('[') => parse_array(chars),
        Some('t') => parse_literal(chars, "true", Value::Bool(true)),
        Some('f') => parse_literal(chars, "false", Value::Bool(false)),
        Some('n') => parse_literal(chars, "null", Value::Null),
        Some(c) if c.is_ascii_digit() || *c == '-' => parse_number(chars),
        other => Err(format!("RJson: unexpected token {other:?}")),
    }
}

fn parse_literal(chars: &mut Peekable<Chars>, literal: &str, value: Value) -> Result<Value, String> {
    for expected in literal.chars() {
        match chars.next() {
            Some(c) if c == expected => {}
            other => return Err(format!("RJson: expected '{literal}', got {other:?}")),
        }
    }
    Ok(value)
}

fn parse_number(chars: &mut Peekable<Chars>) -> Result<Value, String> {
    let mut buf = String::new();
    while matches!(chars.peek(), Some(c) if c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')) {
        buf.push(chars.next().unwrap());
    }
    buf.parse::<f64>().map(Value::Number).map_err(|e| format!("RJson: invalid number '{buf}': {e}"))
}

fn parse_string(chars: &mut Peekable<Chars>) -> Result<String, String> {
    if chars.next() != Some('"') {
        return Err("RJson: expected opening '\"'".to_string());
    }
    let mut out = String::new();
    loop {
        match chars.next() {
            None => return Err("RJson: unterminated string".to_string()),
            Some('"') => return Ok(out),
            Some('\\') => match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('/') => out.push('/'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('b') => out.push('\u{8}'),
                Some('f') => out.push('\u{c}'),
                Some('u') => {
                    let code = parse_hex4(chars)?;
                    // サロゲートペア(絵文字等)の最小限の処理: 高位サロゲートに
                    // 続けて`\uXXXX`低位サロゲートが来る場合のみ結合する。
                    if (0xD800..=0xDBFF).contains(&code) {
                        if chars.next() == Some('\\') && chars.next() == Some('u') {
                            let low = parse_hex4(chars)?;
                            let c = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                            if let Some(ch) = char::from_u32(c) {
                                out.push(ch);
                            }
                        }
                    } else if let Some(ch) = char::from_u32(code) {
                        out.push(ch);
                    }
                }
                other => return Err(format!("RJson: invalid escape {other:?}")),
            },
            Some(c) => out.push(c),
        }
    }
}

fn parse_hex4(chars: &mut Peekable<Chars>) -> Result<u32, String> {
    let mut hex = String::with_capacity(4);
    for _ in 0..4 {
        hex.push(chars.next().ok_or("RJson: truncated \\u escape")?);
    }
    u32::from_str_radix(&hex, 16).map_err(|e| format!("RJson: invalid \\u escape '{hex}': {e}"))
}

fn parse_array(chars: &mut Peekable<Chars>) -> Result<Value, String> {
    chars.next(); // '['
    let mut items = Vec::new();
    skip_whitespace(chars);
    if chars.peek() == Some(&']') {
        chars.next();
        return Ok(Value::Array(items));
    }
    loop {
        items.push(parse_value(chars)?);
        skip_whitespace(chars);
        match chars.next() {
            Some(',') => continue,
            Some(']') => break,
            other => return Err(format!("RJson: expected ',' or ']' in array, got {other:?}")),
        }
    }
    Ok(Value::Array(items))
}

fn parse_object(chars: &mut Peekable<Chars>) -> Result<Value, String> {
    chars.next(); // '{'
    let mut fields = Vec::new();
    skip_whitespace(chars);
    if chars.peek() == Some(&'}') {
        chars.next();
        return Ok(Value::Object(fields));
    }
    loop {
        skip_whitespace(chars);
        let key = parse_string(chars)?;
        skip_whitespace(chars);
        match chars.next() {
            Some(':') => {}
            other => return Err(format!("RJson: expected ':' after key, got {other:?}")),
        }
        let value = parse_value(chars)?;
        fields.push((key, value));
        skip_whitespace(chars);
        match chars.next() {
            Some(',') => continue,
            Some('}') => break,
            other => return Err(format!("RJson: expected ',' or '}}' in object, got {other:?}")),
        }
    }
    Ok(Value::Object(fields))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_string_array() {
        let v = parse(r#"["a.git","b.git"]"#).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str(), Some("a.git"));
        assert_eq!(arr[1].as_str(), Some("b.git"));
    }

    #[test]
    fn parses_object_with_escapes_and_unicode() {
        let v = parse(r#"{"branch":"main","content":"line1\nline2 \"quoted\" 日本"}"#).unwrap();
        assert_eq!(v.get("branch").and_then(Value::as_str), Some("main"));
        assert_eq!(v.get("content").and_then(Value::as_str), Some("line1\nline2 \"quoted\" 日本"));
    }

    #[test]
    fn parses_empty_array_and_object() {
        assert!(parse("[]").unwrap().as_array().unwrap().is_empty());
        assert!(matches!(parse("{}").unwrap(), Value::Object(f) if f.is_empty()));
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(parse("{\"a\":").is_err());
        assert!(parse("[1,2,").is_err());
        assert!(parse("\"unterminated").is_err());
    }
}
