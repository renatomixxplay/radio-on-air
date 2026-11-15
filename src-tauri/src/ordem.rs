//! Ordem alfabética "natural": `2 Abertura` vem antes de `10 Fechamento`,
//! sem diferenciar maiúsculas e acentos.

use deunicode::deunicode;
use std::cmp::Ordering;

pub fn natural(a: &str, b: &str) -> Ordering {
    let (a, b) = (deunicode(a).to_lowercase(), deunicode(b).to_lowercase());
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(c1), Some(c2)) if c1.is_ascii_digit() && c2.is_ascii_digit() => {
                let n1: String = std::iter::from_fn(|| x.next_if(|c| c.is_ascii_digit())).collect();
                let n2: String = std::iter::from_fn(|| y.next_if(|c| c.is_ascii_digit())).collect();
                let (t1, t2) = (n1.trim_start_matches('0'), n2.trim_start_matches('0'));
                let ord = t1.len().cmp(&t2.len()).then_with(|| t1.cmp(t2));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(c1), Some(c2)) => {
                if c1 != c2 {
                    return c1.cmp(&c2);
                }
                x.next();
                y.next();
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn numeros_em_ordem_natural() {
        let mut v = vec!["10 Fechamento", "2 Ofertas", "1 Abertura", "Água", "agua 3"];
        v.sort_by(|a, b| natural(a, b));
        assert_eq!(v, ["1 Abertura", "2 Ofertas", "10 Fechamento", "Água", "agua 3"]);
    }
}
