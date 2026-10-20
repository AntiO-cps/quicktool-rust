#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    Op(char),
}

pub fn calculate(input: &str, locale: &str) -> Result<f64, String> {
    let decimal = if ["de", "fr", "es", "it", "ru"]
        .iter()
        .any(|p| locale.starts_with(p))
    {
        ','
    } else {
        '.'
    };
    let group = if decimal == ',' { '.' } else { ',' };
    let chars: Vec<char> = input.trim().chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || c == decimal || (decimal != '.' && c == '.') {
            let mut raw = String::new();
            let mut dot = false;
            while i < chars.len() {
                let c = chars[i];
                if c.is_ascii_digit() {
                    raw.push(c);
                    i += 1;
                } else if !dot && (c == decimal || (decimal != '.' && c == '.')) {
                    raw.push('.');
                    dot = true;
                    i += 1;
                } else if c == group
                    && chars
                        .get(i + 1..i + 4)
                        .is_some_and(|s| s.iter().all(char::is_ascii_digit))
                    && !chars.get(i + 4).is_some_and(char::is_ascii_digit)
                {
                    i += 1;
                } else {
                    break;
                }
            }
            if raw.ends_with('.') {
                return Err("无法识别数字".into());
            }
            tokens.push(Token::Number(raw.parse().map_err(|_| "无法识别数字")?));
        } else if c.is_ascii_alphabetic() || c == '_' || c == 'π' {
            let start = i;
            i += 1;
            while chars
                .get(i)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == 'π')
            {
                i += 1;
            }
            tokens.push(Token::Name(
                chars[start..i]
                    .iter()
                    .collect::<String>()
                    .to_lowercase()
                    .replace('π', "pi"),
            ));
        } else if "+-*/%^(),;!".contains(c) {
            tokens.push(Token::Op(c));
            i += 1;
        } else {
            return Err(format!("不支持的字符：{c}"));
        }
    }
    if tokens.is_empty() {
        return Err("请输入表达式".into());
    }
    let mut p = Parser {
        tokens,
        pos: 0,
        depth: 0,
    };
    let value = p.expr()?;
    if p.pos != p.tokens.len() || !value.is_finite() {
        return Err("表达式或计算结果无效".into());
    }
    Ok(value)
}
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    depth: usize,
}
impl Parser {
    fn eat(&mut self, c: char) -> bool {
        if self.tokens.get(self.pos) == Some(&Token::Op(c)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn primary(&mut self) -> Result<f64, String> {
        self.depth += 1;
        if self.depth > 128 {
            return Err("表达式嵌套过深".into());
        }
        let token = self.tokens.get(self.pos).cloned().ok_or("表达式不完整")?;
        self.pos += 1;
        let result = match token {
            Token::Number(v) => Ok(v),
            Token::Op('(') => {
                let v = self.expr()?;
                if !self.eat(')') {
                    return Err("括号不匹配".into());
                }
                Ok(v)
            }
            Token::Name(n) => {
                if self.eat('(') {
                    let mut args = Vec::new();
                    if !self.eat(')') {
                        loop {
                            args.push(self.expr()?);
                            if self.eat(',') || self.eat(';') {
                                continue;
                            }
                            if !self.eat(')') {
                                return Err("函数括号不匹配".into());
                            }
                            break;
                        }
                    }
                    function(&n, &args)
                } else {
                    match n.as_str() {
                        "pi" => Ok(std::f64::consts::PI),
                        "tau" => Ok(std::f64::consts::TAU),
                        "e" => Ok(std::f64::consts::E),
                        _ => Err(format!("未知常量：{n}")),
                    }
                }
            }
            _ => Err("表达式语法错误".into()),
        };
        self.depth -= 1;
        result
    }
    fn postfix(&mut self) -> Result<f64, String> {
        let mut v = self.primary()?;
        while self.eat('!') {
            if v.fract() != 0. || !(0.0..=170.).contains(&v) {
                return Err("阶乘仅支持 0 至 170 的整数".into());
            }
            v = (2..=v as u32).fold(1., |a, b| a * b as f64);
        }
        Ok(v)
    }
    fn unary(&mut self) -> Result<f64, String> {
        if self.eat('-') {
            Ok(-self.postfix()?)
        } else {
            self.eat('+');
            self.postfix()
        }
    }
    fn power(&mut self) -> Result<f64, String> {
        let v = self.unary()?;
        if self.eat('^') {
            self.depth += 1;
            if self.depth > 128 {
                return Err("表达式嵌套过深".into());
            }
            let rhs = self.power()?;
            self.depth -= 1;
            Ok(v.powf(rhs))
        } else {
            Ok(v)
        }
    }
    fn term(&mut self) -> Result<f64, String> {
        let mut v = self.power()?;
        loop {
            if self.eat('*') {
                v *= self.power()?
            } else if self.eat('/') {
                v /= self.power()?
            } else if self.eat('%') {
                v %= self.power()?
            } else if matches!(
                self.tokens.get(self.pos),
                Some(Token::Number(_) | Token::Name(_) | Token::Op('('))
            ) {
                v *= self.power()?
            } else {
                break;
            }
        }
        Ok(v)
    }
    fn expr(&mut self) -> Result<f64, String> {
        let mut v = self.term()?;
        loop {
            if self.eat('+') {
                v += self.term()?
            } else if self.eat('-') {
                v -= self.term()?
            } else {
                break;
            }
        }
        Ok(v)
    }
}
fn function(name: &str, a: &[f64]) -> Result<f64, String> {
    let first = || a.first().copied().ok_or("函数缺少参数".to_string());
    let v = match name {
        "min" => a.iter().copied().reduce(f64::min).ok_or("函数缺少参数")?,
        "max" => a.iter().copied().reduce(f64::max).ok_or("函数缺少参数")?,
        "hypot" => {
            let maximum = a.iter().map(|v| v.abs()).fold(0., f64::max);
            if maximum == 0. {
                0.
            } else {
                maximum
                    * a.iter()
                        .map(|v| (v / maximum).powi(2))
                        .fold(0., |sum, n| sum + n)
                        .sqrt()
            }
        }
        "pow" => {
            if a.len() < 2 {
                return Err("pow 需要两个参数".into());
            }
            a[0].powf(a[1])
        }
        "sin" => first()?.sin(),
        "cos" => first()?.cos(),
        "tan" => first()?.tan(),
        "asin" => first()?.asin(),
        "acos" => first()?.acos(),
        "atan" => first()?.atan(),
        "sqrt" => first()?.sqrt(),
        "abs" => first()?.abs(),
        "floor" => first()?.floor(),
        "ceil" => first()?.ceil(),
        "round" => {
            let value = first()?;
            if (-0.5..0.).contains(&value) || value == -0.5 {
                -0.
            } else {
                (value + 0.5).floor()
            }
        }
        "ln" => first()?.ln(),
        "log" | "log10" => first()?.log10(),
        "exp" => first()?.exp(),
        _ => return Err(format!("未知函数：{name}")),
    };
    if v.is_finite() {
        Ok(v)
    } else {
        Err("计算结果无效".into())
    }
}
