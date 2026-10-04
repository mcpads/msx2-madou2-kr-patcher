use super::ending;

#[test]
fn classifies_korean_endings_and_digits_without_guessing_latin_pronunciation() {
    for c in "가나루셰죠사탄".chars() {
        assert_eq!(ending(c), if c == '탄' { 2 } else { 1 });
    }
    for c in "각값힣".chars() {
        assert_eq!(ending(c), 2);
    }
    for c in "013678".chars() {
        assert_eq!(ending(c), 2);
    }
    for c in "2459".chars() {
        assert_eq!(ending(c), 1);
    }
    for c in " …!?」)".chars() {
        assert_eq!(ending(c), 0);
    }
    for c in "AZéㄱ".chars() {
        assert_eq!(ending(c), 3);
    }
}
