# 원본 문단 구조를 유지하는 Enter 입력

r208의 양 OS 고정 검증을 완료했다. 직전 채택은 [r207](0195-worksheet-text-density.md)이다. GUI·스크린샷·설치 Mathcad 실행을 사용하지 않는다.

## 원본 근거와 범위

[정적 REA](../../artifacts/rea/rich-text-paragraph-next-20261020.md)는 설치 CIL45개 body와 XAML을 고정했다. Enter의 Paragraph 분할과 Shift+Enter의 LineBreak는 구별된다. 문단 병합의 wrapper·서식 상속 및 Backspace/Delete 처리도 다르다. 단순히 LF를 Run에 넣는 것으로 원본 문단 편집을 구현했다고 볼 수 없다.

이번 범위는 이미 검증된 direct Section/FlowDocument→Paragraph→Run의 비어 있지 않은 Run 내부, 접힌 caret에서 Enter 분할이다. 원래 Paragraph/Run의 opening/closing bytes와 서식·언어·메타데이터를 보존한다. 새 오른쪽 문단에 입력이 이어지고 반복 분할, 원래 여러 문단, 한글·emoji, Undo/Redo, 자체 저장과 native export 후 cold reopen을 지원한다. 선택 범위 분할·빈 문단·Run 경계·문단 병합·Shift+Enter·newline paste·rich 선택 clipboard는 아직 지원하지 않는다. 미지원 입력은 원문과 history를 바꾸기 전에 실패한다.

## 소스 모델과 비용

optional compact `paragraph_origins`는 현재 문단과 원본 문단의 관계를 보존한다. 기존 문서는 필드가 없으며 기존 JSON·시각 signature·비용 계약을 유지한다. map은 원본 전체를 순서대로 빠짐없이 덮으며 연속 복제만 허용한다. explicit null·빈 map·중복 필드·비정수·범위 초과는 거절한다. 현재 문단1024개, Run4096개, 소스1MiB 및 문서8MiB 한도에 map 비용도 포함한다.

기존 XML parse 때 문단 범위를 함께 저장한다. 분할 검증에 추가 전체 XML parse3회를 넣지 않는다. 원본 wrapper 밖 bytes와 문단 metadata 관계를 검증한 뒤 패치를 만들며, 새 wire와 plain·caret·history는 모든 한도를 확인한 뒤 설치한다. native에는 현재 XAML만 출력하고 재열기 때 새 원본 권위를 생성한다.

IME preedit와 확인 Enter를 구조 편집으로 해석하지 않는다. 지연 입력 queue가 같은 native 입력 batch의 IME 확인 provenance를 보존하고, 다음 독립 Enter에는 이를 넘기지 않는다. Run 귀속·색상·기존 B/I/크기와 계산 비실행 계약을 유지한다.

## 개발 검사

[코어 수정 후41검사](../../artifacts/rea/paragraph-r208-core-fixture-repair-20261020.log), [실제 Enter3검사](../../artifacts/rea/paragraph-r208-enter-smoke-preflight-20261020.log), [앱 통합5검사](../../artifacts/rea/paragraph-r208-integration-path-repair-20261020.log), [queue17검사](../../artifacts/rea/paragraph-r208-input-provenance-preflight-20261020.log)가 통과했다. 초기 실패 기록은 보존했다. explicit Normal 서식이 없는 테스트 fixture, 기존 CDATA 거절 조건과 맞지 않던 oracle, native import의 기존 pathNone 정책을 잘못 가정한 테스트를 고쳤다. parser나 저장 정책을 느슨하게 만들지 않았다.

[결과 보존 회귀](../../artifacts/rea/paragraph-r208-result-proof-repair-preflight-20261020.log)는 텍스트 문단 편집을 format-only라고 판정하지 않으며 숫자2개와 plot 결과의 기존 payload bytes를 유지한 채 Pending으로 저장함을 확인한다. 텍스트만 있는 probe로 계산 결과 보존을 주장하지 않는다.

[기능 probe](../../artifacts/rea/paragraph-r208-artifact-preflight-20261020.log)는 native5개·자체5개·관측10상태를 생성했다. [최신 Clippy](../../artifacts/rea/paragraph-r208-clippy-final-preflight-20261020.log)도 통과했다. 새 native5개의 PDF 출력은 성공했으며 독립 파일 검증과 동결된 양 OS 전체 검사는 아래 최종 결과로 완료했다. 이 문서는 완성률 수치나 원본 runtime 재열기 성공을 뜻하지 않는다.

## 양 OS 고정 검증

[최종 검증](../../artifacts/rea/paragraph-integration-20261020-r208/validation.json)은 checkpoint_accepted/current_product_matches_archive 모두 true다. 개발 실패는 별도 보존했으며 최종 gate/probe/validator 재실행은 없다. 설치 Mathcad에서 재열기는 검증하지 않았다.

| 항목 | 결과 |
|---|---|
| source archive | 14,008,320 bytes · SHA `8aedc61491772a71205f3937b6d5ef988abad17521ac86dcdce9f473b2fd09a4` |
| manifest | 561 files / Rust296 · SHA `1778cfdff56e7648099ffd272e8b55a636515a5d9783976fad34b11bd045a456` |
| Windows / peets | 각각 1733 PASS / 실패0 · ignored93/92(Windows 전용 그래픽 조회1개 차이) |
| format / Clippy / build / reference | 양 OS PASS · Windows release PASS |
| MCDX / XML / PDF | 152 / 10 / 80 · XSD922parts · image0 · fitz+pypdf PASS |
| 실행 파일 | [r208](../../artifacts/rea/mathcraft-paragraph-r208-20261020.exe) · 25,406,976 bytes · SHA `a368ad5d28378bffae94a0e36ddf97f41be7a2f9007f9f13eae963c6bda20a73` |
| validation | SHA `c5ff37e34ce433e50aaa500de154b24786a25ed838815e81ddd79b565b2c8372` |

HD035/D08의 부분 구현 깊이를 갱신한다. 세부555행은 부분156/미구현399, 상위87행은 구현4/부분69/미구현14다. 이를 완성률로 환산하지 않는다.
