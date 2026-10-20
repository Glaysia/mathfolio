# EA-012: 진법 입력·네이티브 숫자·계산 경계

2026-10-19. 설치된 Mathcad Prime 12.0.0.1의 정적 REA/CIL·XSD·도움말 근거와 현재 통합 코드를 구분한다. 원본 GUI·스크린샷·CLR·COM·계산 엔진은 실행하지 않았다. 현재 main에 통합된 범위의 최종 양 OS 게이트는 아직 진행 전이거나 진행 중이며, 이 문서는 완성률 또는 전체 원본 동작 일치 판정이 아니다.

## 근거와 식별자

- [원본 모드·직렬화 계약](../../artifacts/rea/radix-literal-mode-next-contract-20261019.md), [51개 메서드 ledger](../../artifacts/rea/radix-literal-mode-ledger-20261019.json), [원본 숫자 메서드 자료](../../artifacts/rea/radix-literal-focused-rea-20261019.json).
- [계산 엔진 경계](../../artifacts/rea/radix-engine-next-contract-20261019.md), [설치 바이트 검증 ledger](../../artifacts/rea/radix-engine-next-contract-20261019.json). 이 자료는 원본 진법 파서의 전체 복원이 아니라 확인된 숫자 저장소와 미확정 사항을 기록한다.
- [오른쪽 끝 추가 입력 계약](../../artifacts/rea/radix-end-append-contract-20261019.md), [23개 메서드 ledger](../../artifacts/rea/radix-end-append-contract-20261019.json), [새 REA 자료](../../artifacts/rea/radix-end-append-fresh-20261019.json). 새 evidence ID는 `ev_6bcfda0fff9cc8cbebeae526fd0d410e7f9340eb7a348b323c84751f19ebed5a`.
- [설정 저장 위치 정정](../../artifacts/rea/string-settings-carrier-erratum-20261019.md), [정정 ledger와 원본 serializer 근거](../../artifacts/rea/string-settings-carrier-erratum-20261019.json). 과거 루트 속성 fixture의 통과를 올바른 원본 carrier 검증으로 재사용하지 않는다.
- [통합 전 코드 감사](../../artifacts/rea/radix-source-audit-20261019.md)는 당시 fail-closed 입력 경로와 캐시 오해석을 기록한 **역사 자료**다. 아래 현재 코드 설명이 이후 변경을 반영한다.

주요 설치 식별자:

| 어셈블리 | SHA256 | MVID |
| --- | --- | --- |
| Ptc.EquationEditor.dll | `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223` | `d8627dca-3258-470c-b4d1-c00bbe2f2441` |
| Ptc.Controls.Core.dll | `654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977` | `b5e8e4fd-c134-4713-b1b1-eea8cddcdd9f` |
| mplrun.dll | `7c5fd2b212b683d5dcfb0e6a7ddf2a0450ccf93b663f70985e2e847d1f9ca798` | 해당 engine ledger 참조 |

메서드 토큰은 이 빌드 안에서만 유효하다. raw decoder의 역사적 `plots` 출력 접두사는 실제 어셈블리 이름이 아니다.

## 원본 옵션과 native carrier

설치 `calculation10.xsd`는 `calculation/calculationBehavior@bin-oct-hex-mode`를 optional boolean, 기본 false로 선언한다. `strings-use-origin`도 같은 직접 자식의 별도 속성이다. 두 옵션은 서로 대체되지 않는다. `<calculation bin-oct-hex-mode="true"/>` 같은 루트 속성은 올바른 carrier가 아니다.

Controls.Core **060009f9**, IL SHA `cd449ad079a0f475abb374b14b941f180e6fe5372c5ba2327b88eb58e91f6f4d`는 `calculationBehavior` 요소와 속성 바인딩을 구성한다. **060009fb**, IL SHA `5f53bd43b6e963e0ef925b02c088b4451c83965982ba7c698b65bbdc120942d0`는 그 객체를 `calculation`의 직접 자식으로 추가한다. [원시 CIL](../../artifacts/rea/feedback/radix-literal-settings-carrier-cil-20261019.txt), [package 연결](../../artifacts/rea/feedback/radix-literal-settings-package-cil-20261019.txt).

원본 리본 BAML은 Calculation Options의 `ToggleBinOctHexModeCalculationOptionCommand`에 연결된다. Worksheet **044f → 048f**, **07d5 → 07d6**은 옵션 변경을 활성 NUMERIC descriptor의 입력 모드에 전달한다. 새 키보드 단축키는 이 근거에서 도출하지 않는다. 기존 native의 명시적 `base`를 읽는 것과 새 접미사 입력을 발견하는 모드는 분리되어 있다.

현재 [native_calculation.rs](../../crates/mathcraft-document/src/native_calculation.rs)는 고유 content-type Override로 활성 설정 part를 찾고, 직접 자식·namespace·중복을 검사한다. relocated part와 UTF-16은 기존 package decoder를 사용한다. 원문에 없는 가짜 속성이나 고정 파일명으로 carrier를 대신하지 않는다.

## 원본 숫자 원문과 writer/reader

설치 도움말 `Entering_binar_octal_hexadecimal_expressions.html`의 SHA는 `867fb429bb7947585ab134f0dc083158e4e744216cf62de76abb5add5efbe7e2`다. 모드를 켠 뒤 `1101b`, `32o`, `2b3ch`, `0a32h`를 입력하는 구문을 설명하고, hex 첫 글자가 A–F이면 앞에 0을 요구한다. 대문자 B/O/H 뒤에는 implied multiplication이 생기며, 다른 문자 사이에도 그 연산자가 생길 수 있다고 설명한다. 따라서 예제 문자열만으로 모든 키별 중간 구조가 자동 합성된다고 결론낼 수 없다.

EquationEditor **0bbb** NUMERIC은 `StringValueNodeInfo`와 원래 lexeme를 사용한다. regex는 decimal/exponent/i/j 외 b/B/o/O/h/H 및 일부 hex 소수 형태도 허용한다. 이 **저작 어휘**는 binary/octal digit 유효성이나 계산 성공을 보장하지 않는다.

원본 writer **119c**, IL SHA `a925fed378b6e4bee31218741e2a79d3674376094faec3b14890bc8c39c48855`:

- 소문자 b/o/h는 마지막 접미사를 떼어 `<real base="2|8|16">digits</real>`로 쓴다. 10진수 계산값으로 원문을 바꾸지 않는다.
- 대문자 suffix NUMERIC은 별도 분기에서 `base` 없이 전체 lexeme를 쓴다. 첫 구현의 소문자 carrier로 무조건 정규화하지 않는다.
- imaginary는 별도 `imag@symbol` 경로다. 이것만으로 진법 imaginary 저작을 지원한다고 주장하지 않는다.

원본 reader **124f**, IL SHA `b91c3c8b9ece27c5dc28fc11060a0c9eee7415569492139fc3e2f9866a4b9e67`와 helper1251은 `base`를 b/o/h 접미사로 되돌려 factory에 넘긴다. 음수는 NEGATION과 양수 원자로 나뉜다. [writer CIL](../../artifacts/rea/feedback/radix-literal-authoring-writer-cil-20261019.txt), [reader CIL](../../artifacts/rea/feedback/radix-literal-native-reader-cil-20261019.txt).

Math50 XSD는 real/imag 모두 optional `base` 기본10, 허용 문자열 2/8/10/16으로 선언한다. 그러나 `numberType`은 최소 길이1인 문자열일 뿐이다. XSD PASS가 digit·부호·소수·범위·정밀도 검증은 아니다. [설치 문서 표본 검색](../../artifacts/rea/radix-shipped-samples-20261019.json)은 77개 mcdx/mctx에서 명시적 base2/8/16 숫자를 찾지 못했다. 위 XML 구문과 새 fixture는 원본 writer/reader에서 도출하거나 직접 구성한 예이며, 원본 계산 결과 캡처로 표기하지 않는다.

현재 [editor radix carrier](../../crates/mathcraft-editor/src/radix_literal.rs)는 `BasedNumber { literal, base }`에 소문자 접미사를 포함한 원문을 저장한다. 기존 Number JSON은 별도다. [bridge](../../crates/mathcraft-compat/src/editor_bridge.rs)와 [두 번째 writer](../../crates/mathcraft-compat/src/editor_bridge/serialization_writer.rs)는 원문 digits/base를 직렬화하며 계산 Quantity를 source로 역투영하지 않는다.

## 계산의 독립적인 명시 범위

원본 mplrun **0561 IntDv**, IL SHA `137fdcfbf32289defc239b816f91bf128d30a454826467ad8d66d023078209f0`의 `conv.r8`과 NumVal의 실수·허수 R8 필드는 일반 숫자 저장소를 확인해 준다. 이것은 원본의 진법 파서·정수 중간 저장형·overflow·tie rounding·소수 계산을 복원한 증거가 아니다. 별도 `Engine.ParseHex`/`StringUtils.ParseInt`를 worksheet 숫자 파서로 연결할 양성 call path도 확보하지 못했다.

현재 [radix_integral.rs](../../crates/mathcraft-compat/src/radix_integral.rs)는 다음 **독립 Rust 지원 정책**을 사용한다.

| 항목 | 명시 지원 또는 경계 |
| --- | --- |
| 진법 | 명시적 real base2/8/16 |
| 어휘 | unsigned ASCII 정수 digit 전체만 허용 |
| 값 | 누적값 `<= 2^53`; 조용한 rounding/wrapping 금지 |
| 크기 | source 최대 65,536바이트; 원본 digit 제한 주장 아님 |
| 긴 leading zero | 숫자 overflow로 오판하지 않고 실제 누적값으로 판단 |
| 소수·내부 부호·separator·잘못된 digit | unsupported; 원문을 단위·이름·10진수로 재해석하지 않음 |
| real의 base 부재/명시10 | 기존 decimal 처리 경계 유지 |
| based imag·미지 속성/namespace | fail closed |

부호 경계는 중요하다. Native bridge는 `real base="2"`의 `-00101`을 읽어 `UnaryNeg(BasedNumber("00101b"))`로 구조화할 수 있고, 다시 쓸 때 neg 적용식으로 표현한다. 반면 raw compat MathML parser는 unsigned decoder에 `-00101`을 직접 넘기는 signed-in-text 기반 숫자를 거절한다. 이미 구조화된 `<apply><neg/><real base="2">00101</real></apply>`는 별도 unary 연산으로 계산한다. 두 경로의 차이를 원본 raw parser 전체 지원으로 확대하지 않는다. unchanged native package 보존과 편집 후 구조적 재직렬화도 바이트 동일성 주장은 다르다.

이전 캐시 reader의 `base` 무시 문제는 현재 [numeric_literal.rs](../../crates/mathcraft-app/src/result_display/numeric_literal.rs)에서 공유 decoder와 속성 검사를 사용하도록 연결되어 있다. 실수·복소수 실수부·행렬 cell·unitedValue 경로가 이를 재사용한다. unsupported based imaginary/미지 메타데이터를 decimal로 표시하지 않고 raw cache는 보존한다. 이 연결은 원본 cached result가 항상 radix를 쓴다는 주장과 별개다.

## `2b` 오른쪽 끝에 `3` 입력: 확인된 중간 구조

NavigatorState **1324**, IL SHA `3efa93935fb11f8f0ef1a044d61d9d94bd7befb023ef6b69e8204adb8a5d9854`는 distinct edge가 없는 value의 끝 offset을 Right1로 분류한다. Normal 입력 **008b → 04d5**는 그러므로 기존 문자열에 단순 append하는 대신 인접 value 삽입을 택한다.

**0491**, IL SHA `b311d98b0803311bac7aa92700b56b39364b860d5858b3dc7e89544281f281f4`는 이전 NUMERIC의 i/j 또는 based suffix를 발견하면 **0492**로 숫자 prefix와 tail을 나눈다. 0492는 `Char.IsDigit` 또는 decimal point를 스캔하며, 모든 문자열에서 단순히 마지막 한 글자만 떼는 함수가 아니다.

따라서 ASCII decimal prefix를 가진 `2b`와 새 `3`의 확인된 분석 중간값은 **기존 숫자2 + 새 식별자b3**다. `3`을 버리거나 binary 원문23으로 만드는 동작은 맞지 않는다. 현재 [입력 adapter](../../crates/mathcraft-app/src/formula/radix_input.rs)는 Normal·선택 없음·끝 위치·순수 ASCII decimal old digits·새 ASCII hex 문자·독립 root/Evaluate operand라는 제한으로 `Scale(Number(2), Identifier(b3))`를 만든다. 임의 ancestor, callee, literal suffix, 부분 선택, typeover, 기존 alpha/fraction/exponent digit까지 이 규칙을 넓히지 않는다.

0491 자체에는 모드/라이선스 guard가 없다. 이후 **0493** 후보 fusion과 **0494**의 based-candidate 검사는 별개다. 원본0494는 mode ON과 유효 라이선스를 검사하며 lowercase를 허용하고 uppercase에는 전달 flag를 사용한다. **라이선스 조건은 관찰한 원본 분기이며 Mathcraft에 재현한 유료 제한이 아니다.**

## 전체 hex의 키별 자동 합성은 미확정

0493은 바로 이전 value와 현재 value만 합쳐 factory·IsValid·0494를 검사한다. **049c**는 suffix 분리0491 다음 fusion0493을 배열하고, **0487**은 노드마다 규칙들을 실행한 뒤 Next로 전진한다. 0493 안에 같은 노드에서 이전 값까지 반복 합치는 루프는 없다. 이후048e의 implicit 제거는 descriptor alternate를 찾는 경로이지 숫자와 식별자의 원문을 합치는 함수가 아니다.

`2`와 `b3ch`가 실제 인접 value라면 전체 `2b3ch`는 NUMERIC hex regex 후보가 된다. 그러나 기존 `Scale(2,b3c)`에서 마지막 h를 입력할 때 어느 단계가 그 Scale을 제거하고 재분석하는지까지 양성으로 복원하지 못했다. 현재 adapter의 직접 Scale-pair hex fusion은 필요한 명시 지원 경로지만, **`0a32h`/`2b3ch` 전체 per-key 전이가 원본과 동일하다는 판정은 UNKNOWN**이다. 이 한계는 도움말 예제나 현재 테스트 통과만으로 지우지 않는다.

partial/mid-body/delete, uppercase provenance, 진법 fraction 계산, 원본 precision/overflow/error resource, 모든 clipboard/다중 문자 입력 및 임의 parent 재분석도 남은 범위다. 독립 decoder와 scratch 테스트 근거는 [분리된 receipt](../../artifacts/rea/radix-engine-next-20261019/receipt.json)에 있으며, 그것을 최종 main 양 OS 게이트나 원본 비교 실행으로 세지 않는다.
