# 마도물어 II (MSX2) 한글 패처

MSX2용 《마도물어 II》 일본판 디스크 이미지(DSK)에 한글 패치를 적용하는 Rust 코드입니다. 원본 식별, 컷신·본편·메뉴·스태프롤 텍스트 재배치, 한글 글꼴 변환과 압축, typed Z80 훅 조립, Expected Write 계획과 최종 diff 검증을 제공합니다.

배포용 BPS와 적용 방법은 [마도물어 시리즈 한글 번역 프로젝트](https://github.com/mcpads/madou-monogatari-kr-patch/tree/main/msx2-madou2)에서 제공합니다. 배포 패치는 베타(v0.1.0)입니다.

## 제공하지 않는 것

이 저장소에는 원본 디스크, 패치를 적용한 디스크, 번역 JSON, 한글 제목 그래픽과 글꼴 파일이 없습니다. 따라서 이 저장소만으로는 배포 패치를 다시 만들 수 없습니다. 아래 입력을 직접 갖춘 경우에만 `build-development`가 디스크를 생성합니다. BPS 생성 기능은 들어 있지 않으므로 BPS는 별도 도구로 만듭니다.

## 빌드와 테스트

```bash
cargo build --release --locked
cargo test --locked
```

기본 테스트는 합성 입력만 사용하며 원본이나 글꼴이 필요한 테스트는 없습니다. 원본·번역·글꼴을 사용하는 검증은 아래 디스크 생성 명령이 수행합니다.

## 지원 원본

| 원본 | 파일명 | 크기 | SHA-256 |
| --- | --- | --- | --- |
| 일본판 | `Madou Monogatari 2.dsk` | 737,280바이트 | `97cd87989e7623e5d6f8aa2d3e0eb98fb318a0d6da190a1563b011e8ff372a8c` |

원본은 첫 번째 인자로 준 디렉터리에서 위 파일명으로 찾습니다. 크기나 SHA-256이 다르면 빌드하지 않습니다. 식별값은 `config/source-media.json`에 있습니다.

## 빌드 입력

| 입력 | 위치 | 비고 |
| --- | --- | --- |
| 원본 디스크 | `roms/Madou Monogatari 2.dsk` | 디렉터리는 첫 번째 인자 |
| 컷신 번역 | `assets/translation/cutscenes.ko.json` | 두 번째 인자 |
| 스태프롤·제목 번역 | 컷신 번역과 같은 디렉터리의 `credits.ko.json`, `title.ko.json` | 파일명 고정 |
| 본편·메뉴 번역 | `assets/translation/main.ko.json`, `menus.ko.json` | 네 번째·다섯 번째 인자 |
| 부팅 안내·직접 표기 번역 | 본편 번역과 같은 디렉터리의 `boot-warning.ko.json`, `direct-labels.ko.json` | 파일명 고정 |
| 제목 그래픽 | 컷신 번역 디렉터리 기준 `../graphics/title/cells.json` | `title.ko.json`에 적힌 SHA-256과 대조 |
| 컷신·스태프롤 글꼴 | Neo둥근모 1.600 `neodgm.ttf` | [Neo둥근모](https://github.com/neodgm/neodgm), 세 번째 인자 |
| 본편·메뉴·부팅 안내 글꼴 | Galmuri7 2.403 `Galmuri7.bdf` | [Galmuri](https://github.com/quiple/galmuri), 여섯 번째 인자 |

번역 JSON은 원본 SHA-256과 원문 참조를 함께 담으며, 형식은 이를 읽는 `src/cutscene_draft.rs`, `src/credits.rs`, `src/title.rs`, `src/encode_main.rs`, `src/menu_draft.rs`, `src/boot_warning.rs`, `src/direct_labels.rs`의 구조체가 정합니다. 배포 패치 v0.1.0은 제목 그래픽 SHA-256 `5161250ca6d96f16e7ab62258104fdb857a40c175224869e8893dffec56a1c12`로 만들었습니다.

글꼴은 재배포 조건을 이 저장소에서 보장할 수 없어 포함하지 않습니다. 각 글꼴의 라이선스는 배포처에서 확인하세요. 빌드는 다음 SHA-256과 다른 글꼴 파일을 거부합니다.

```text
d61b60eccb731f8ca9c7da582e4a05a94db66b570471809950aa9a7261b941d6  neodgm.ttf
2a6fd090ac6d24f7392d6cc49db02ce54b9d1c01048bf8c97cbcdbc5a885cb15  Galmuri7.bdf
```

## 디스크와 패치 생성

```bash
cargo run --release --locked -- build-development roms \
  assets/translation/cutscenes.ko.json path/to/neodgm.ttf \
  assets/translation/main.ko.json assets/translation/menus.ko.json \
  path/to/Galmuri7.bdf out/madou2-ko.dsk out/build.json
```

마지막 두 인자는 생성할 디스크와 빌드 보고서입니다. 빌드는 원본 식별, 제어 코드·문자 매핑·용량·포인터를 검사하고 모든 변경을 하나의 Expected Write 계획으로 적용한 뒤 허용 범위와 전체 diff를 검증합니다.

배포 패치 v0.1.0을 적용한 디스크의 SHA-256은 `e38a968a2bcef8c850537a56fca44bc211c07eb282864c49748df05e68a857f8`입니다. BPS는 원본 DSK 전체와 생성한 DSK로 BPS 생성 도구(예: Flips)에서 만듭니다. 생성 도구에 따라 BPS 파일 자체의 바이트는 달라질 수 있으므로 적용 결과 디스크의 SHA-256으로 비교합니다.

## 그 밖의 명령

인자 없이 실행하면 전체 사용법이 나옵니다. `build-cutscenes`, `inspect-cutscenes`, `encode-main`, `compile-font`과 각종 `*-probe` 명령은 부분 설치와 중간 결과 확인용입니다.

## 라이선스

이 저장소의 소스 코드는 [MIT License](LICENSE)로 제공합니다.
