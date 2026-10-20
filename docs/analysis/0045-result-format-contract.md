# 결과 서식 명령·MCDX 저장 계약

2026-10-16. Mathcad Prime 12.0.0.1의 원본 기능 조사다. 기존 [0022](0022-math-formatting.md)의 숫자 서식 조사에서 미연결 상태였던 Percent·복소수 표시·추가 서식 옵션을 확인했다. 보존한 REA 메타데이터, SHA를 재검증한 CIL, 원본 BAML 및 설치 도움말만 사용했다. GUI·스크린샷·원본 앱·CLR·COM 실행은 사용하지 않았다. 이 문서의 관찰은 Rust 구현 전체의 완료·동등성 주장이 아니다.

## 원본 리본과 변경 경로

원본의 **Math Formatting → Results**, 한국어 **계산 서식 지정 → 결과** 그룹에 이미 있는 기능이다. 새 기능으로 제안할 대상이 아니다.

한국어 satellite `MathcadPrime.resources.dll`의 SHA-256은 `a511bb8b8c39391d107cbd3f5409e08ab5f258c42588dbd06f5d9acfd9ea457f`, REA Evidence는 `ev_2e64a3c5431a2fbcaea4d8a711761138916f0159a5ad240aeece5c0e7e48562a`다. inert ResourceReader/BAML 디코더로 다음 원본 리소스의 구조·command·property binding을 읽었다.

| 리소스 | SHA-256 | 확인한 내용 |
| --- | --- | --- |
| `mathformatwithdebugtab.baml` | `341227e18c291e5ea590cf296d7022126c7078e027ea8e0763f6172fa483dd16` | 탭 제목 계산 서식 지정, NumberFormatRibbonBar 그룹 제목 결과 |
| `numberformatribbonbar.baml` | `eb719f2563341bf20abba8b4c3201c1cad377a558bc497999d42b5097dbe50e6` | 결과 형식, 기수, 정밀도 표시, 뒤에 오는 0 표시, 복소수 값, 세 분계점과 서식 지우기 |

결과 형식 dropdown의 값은 `General`, `Decimal`, `Scientific`, `Engineering`, `Percent` 순서다. 복소수 값 dropdown은 별도의 `ComplexStyle` property를 바꾸며 `CartesianI`, `CartesianJ`, `PolarPiRadians`, `PolarRadians`, `PolarDegrees` 순서다. 따라서 General/Percent 같은 수치 표시 방식과 직교·극형식은 서로 배타적인 단일 옵션 목록이 아니다.

위 컨트롤은 `NumberFormat.SetNumberFormatFieldCommand`와 `SetPropertyValueCommandParameter`로 연결된다. `EquationControl.OnSetNumberFormatField` (`0x06000178`, IL SHA `3a8ad1ca3d8a019c619e8147861d419132be4e87203baac9d64f6667cb2234fd`)는 상속된 DisplayNumberFormat을 필요한 경우 복제해 영역의 local value로 만든 다음 property를 undoable command로 바꾼다. `HandleResultFormattingChanged` (`0x06000235`, SHA `2b166635a7d25e9c4225a167f98ca5386110d85fae47ca95c3ebda302fa887c2`)는 이 표시 NumberFormat을 수식 모델의 ResultNumberFormat에 전달한다. 수학 AST를 새 수식으로 고치는 기능이 아니다.

분계점 컨트롤은 같은 그룹 안의 `ComplexThresholdUpDown`, `ZeroThresholdUpDown`, `ExponentialThresholdUpDown`이고 각각 범위가 0–307, 0–307, 0–15다. 설치 영문 도움말은 같은 Results 그룹의 Threshold 메뉴로 설명한다. 지수 분계점은 General 형식일 때 활성화하는 converter에 연결된다. ExponentStyle의 E notation은 아래 저장·formatter 지원이 있지만, 이번 결과 리본 BAML과 설치 결과 서식 도움말에서는 별도 변경 버튼의 위치를 확인하지 못했다. 저장 지원을 이유로 새 UI 버튼을 발명하지 않는다.

## Percent는 결과 표시 형식이다

원본 `NumberStyles` 상수는 Engineering=0, Scientific=1, Decimal=2, Binary=3, Octal=4, Hexadecimal=5, **Percent=6**, General=7이다. Binary/Octal/Hexadecimal의 enum 존재를 결과 형식 dropdown 항목 존재로 확대하면 안 된다. 리본은 별도 Radix 컨트롤을 사용한다.

`RealNumberFormatter.FormatRealToPercent` (`0x06000596`, IL SHA `6f1840caa35a1a10648f9223193250afb17e400d72a66074866edfc377298476`)는 값을 100배 해 표시하고 Percent helper에 전달한다. `FormatReal` (`0x0600058b`)은 원래 값에 ZeroThreshold를 먼저 적용한 뒤 Percent 등 스타일로 분기한다. 표시 서식으로 엔진에 저장한 값이나 후속 계산의 값을 100배 바꾸지 않는다.

설치 `percent_operator.html`은 입력 `%` 연산자가 값을 0.01배 한다는 별도 의미와, 결과를 percentage로 보려면 Results의 Result Format에서 Percent를 고르는 조작을 함께 설명한다. 파일 SHA는 `c01114f3dec5e3103a7ac9bfe7c0961e5dd59ce9d781c50760a56f8cc01f33bf`다. 따라서 결과 서식 Percent를 evaluation의 unitOverride나 `%` 입력 연산자로 대체하는 것은 원본 저장 계약과 다르다.

## 복소수와 극형식

`ComplexStyles` 상수와 MCDX `imaginary-value` 대응은 다음과 같다. getter `0x06000b5e`와 setter `0x06000b5f`의 switch/string 분기가 이를 직접 입증한다.

| ComplexStyles | 값 | imaginary-value |
| --- | --- | --- |
| CartesianI | 0 | `i` |
| CartesianJ | 1 | `j` |
| PolarRadians | 2 | `polarRadians` |
| PolarPiRadians | 3 | `polarPiRadians` |
| PolarDegrees | 4 | `polarDegrees` |

`NumberFormat.ApplyComplexThreshold` (`0x060004d2`, SHA `6351d5305edab457f0ff9a397e4ebf719e50c12aa5ddc160b5408caa38d4db15`)는 두 성분이 모두 nonzero일 때 `abs(re/im) > 10^t`이면 허수부를 0으로 하고, 그렇지 않고 `abs(im/re) > 10^t`이면 실수부를 0으로 한다. 비교는 strict greater-than이므로 경계의 같은 값은 유지한다. 계산값을 변경하는 것이 아니라 표시를 위한 성분 억제다.

EquationEditor의 formatter `ah.c` (`0x060003a9`)는 이 상대 성분 분계점을 적용한 뒤 Cartesian/Polar로 나눈다. Polar `ah.a` (`0x060003ab`, SHA `0cbcac2004c9a40c93ec9f62664e07e30d04fb93931d874d9e545fbad264dbb4`)는 크기 `sqrt(re²+im²)`와 각 `atan2(im,re)`를 계산해 **POLAR(크기, 각)** 표시 노드를 만든다. radians는 각 그대로, pi-radians는 `SCALE(각/π, π)`, degrees는 `DEGREE(각/π×180)` 노드를 사용한다. 크기의 표시 노드가 0이면 각을 붙이지 않는다. 극형식은 `exp(iθ)`로 수식을 다시 쓰는 기능이 아니다.

이 Polar 메서드에서 크기와 각에 쓰는 숫자 formatter는 모두 같은 `ah.f(double)` (`0x0600039f`, SHA `e719a83c4148d13e523f06effff4f1ce4369a8c0627c86677cd2d1188abe26fd`)다. 현재 NumberStyle이 Percent이면 둘 다 `0x060003ac`의 100배 Percent 표시로 분기한다. 각에만 General/Decimal을 강제하는 코드는 이 경로에 없다. 따라서 Percent+Degrees 조합의 formatter AST가 `DEGREE(PERCENT(9000))`처럼 되는 경우를 보기 좋다는 이유로 `90°`로 고칠 근거는 없다. 최종 descriptor builder/typesetter가 모든 조합을 어떻게 축약하는지는 실행으로 관찰하지 않았다.

Cartesian formatter (`0x060003aa`)는 실수부·허수부를 따로 표시한 뒤 덧셈/뺄셈 노드로 묶고, 표시상 0인 성분을 생략한다. decimal 허수 formatter (`0x060003a7`)는 1이라는 계수를 자동으로 지우지 않고 숫자 문자열 뒤에 i/j를 붙인다. Percent 허수 formatter (`0x060003ad`)는 `PERCENT(숫자문자열+i/j)`를 만든다. 따라서 복소 Percent는 각 성분을 따로 percentage 표시하며, 단일 괄호 전체 뒤에 %를 붙이는 AST라는 근거는 없다. π의 계수 1 생략을 포함한 모든 downstream typesetter 축약은 이번 조사의 범위 밖이다.

## MCDX의 정확한 형식과 원본 writer 범위

저장 컨테이너는 worksheet namespace의 `<math><resultFormat>…</resultFormat></math>`다. 수식 MathML과 나란한 표시 설정이다. 원본 `GetRegionXmlNode` (`EquationControl 0x060000f0`)는 local DisplayNumberFormat/ResultMatrixFormat이 있고 expressions-only 모드가 아닐 때 이를 쓴다. schema는 `worksheet50.xsd`, SHA `b4a73084e11db4dbcba52c603e2791a3026b088e150cb482114b9a315f355ba3`다.

| child | 원본 writer가 실제 쓰는 attributes |
| --- | --- |
| `general` | `precision`, `show-trailing-zeros`, `radix`, `zero-threshold`, `imaginary-value`, `exponential-threshold`, `complex-threshold` |
| `decimal` | `precision`, `show-trailing-zeros`, `radix`, `imaginary-value` |
| `scientific` | `use-e-notation`, `precision`, `show-trailing-zeros`, `radix="dec"`, `imaginary-value` |
| `engineering` | `use-e-notation`, `precision`, `show-trailing-zeros`, `radix="dec"`, `imaginary-value` |
| `percent` | `precision`, `show-trailing-zeros`, `radix="dec"`, `imaginary-value` |

`WritePercent` (`0x06000b73`, SHA `bbc6ba966d26322ca4059138bce0ead01cadbb9be0ed0bf75c47f738d2c267c4`)와 `ReadPercentXml` (`0x06000b67`, SHA `17f8bb89599236091623934f4a37cbe2389782b25888ede993550655cd0b8f19`)는 이 독립 `<percent>` child를 처리한다. reader는 precision/trailing-zeros/imaginary-value를 읽고 radix를 decimal로 고정한다. `imaginary-value`는 위의 복소 스타일 5개를 모두 허용한다.

XSD common attrs는 모든 형식에 zero-threshold/complex-threshold를 허용하지만 위 원본 writer는 General 이외에는 이를 쓰지 않는다. XSD General에 `use-engineering-format`도 허용되어 있으나, 이번 빌드의 `WriteGeneral`/`ReadGeneralXml`에는 이 attribute 처리가 없다. 이를 Scientific에 붙이거나 원본 UI 기능으로 단정하지 않는다. 수용 가능한 schema, 실제 원본 저장기, 화면의 컨트롤을 구분한다.

precision의 특수 문자열은 `round-trip-decimal-places`이고 모델 값 -1에 해당한다. precision·서식이 생략된 상태는 worksheet defaults를 상속할 수 있으므로 concrete local default로 무조건 덮어쓰지 않는다. 기존 XML의 미지원 속성·matrix formatting도 지원하는 옵션 하나를 바꿨다는 이유로 삭제하면 안 된다. worksheet 전체 기본값과 영역 local override, mixed 선택·Clear Format의 모든 수명 주기까지 단순 per-region 필드로 완성됐다고 주장할 수는 없다.

## 분계점의 영역 설정·문서 설정과 재열기

분계점 command가 항상 worksheet global로 향하는 것은 아니다. 위 `OnSetNumberFormatField`는 property가 ZeroThreshold/ComplexThreshold여도 활성 영역의 상속 DisplayNumberFormat을 복제해 local override를 만든다. Worksheet의 `HandleNumberFormatChangeFromLimbo` (`0x06000484`, SHA `43c033efdb29ccb6085ac9cb218a013ef1ae00740b74c02c110d90d549e69637`)는 CurrentElement가 선택 영역 collection이면 그 영역들에 command를 dispatch한다. 그렇지 않을 때 worksheet DisplayNumberFormat을 복제·수정하는 별도 경로를 사용한다. 사용자 선택 문맥에 따라 영역 서식과 문서 기본값이 갈린다.

재열기 `EquationControl.SetRegionResultFormat` (`0x060000e2`, SHA `0e00c676d5f5970b8c7deeacc99614a5fefb51f388323d4fe03eaf999541c3d4`)는 현재 DisplayNumberFormat이 있으면 Clone하고 없으면 새 기본 NumberFormat을 만든 뒤 ResultFormat에 넘긴다. `ResultFormat.ReadXml` (`0x0600064c`, SHA `9f6a617ebbd5460d7bf2ef1fe231eaf76b5d1890ab0ecb151066bf3eb53364b7`)은 이 기존 NumberFormat에 NumberFormatSerializer를 적용한다. 따라서 해당 child reader가 읽지 않는 속성은 local XML에서 복원되는 것이 아니라 기존 상속/기본 값으로 남는다.

이 경로만 놓고 보면, Percent·Decimal 등의 지역 ZeroThreshold/ComplexThreshold 변경은 실행 중 local NumberFormat에 존재하지만 앞 표의 writer가 그 값을 쓰지 않으므로 `<percent>`/`<decimal>`을 저장·읽는 것만으로 local 값을 복원할 수 없다. 문서 기본값과 같은 값이라면 재열기 뒤에도 같을 수 있으나 별도 지역값의 보존이 입증되는 것은 아니다. 이 사실을 해결하기 위해 모든 threshold를 원본과 달리 global command로 바꾸면 안 된다. 별도 sidecar가 지역 값을 저장하는지까지 완전히 조사하지 않았으므로 원본 전체의 저장 손실을 런타임으로 확정한 것은 아니다. 독립 구현은 자체 문서에서 전체 설정을 보존하는 것과, 원본 writer/reader가 실제 사용하는 MCDX 필드를 구분해야 한다.

## 도움말·schema·실제 코드의 경계 불일치

설치 `to_format_the_result_display.html` SHA는 `e047ad6a8bf5896306a18d19fb6f40fff012ae12587089e98a0a6320fc4b1901`이다. 도움말은 decimal places 범위를 0–16이라 설명한다. 그러나 실제 `NumberFormat.IsValidDecimalPlaces` (`0x060004b9`, SHA `ae72f89d88c4712b7782ba37d4a4f7c278b538ef16ef691f6229db37ebc729b4`)는 **0–15 또는 sentinel -1**, 그리고 nullable inherited state를 허용한다. 이번 빌드 호환 UI의 일반 정밀도 상한은 15를 유지하는 쪽에 직접 코드 근거가 있다. 16을 지원한다고 도움말 문장 하나로 단정하지 않는다.

ExponentialThreshold도 XSD의 허용 상한 17과 리본 numeric control의 상한 15가 다르다. schema가 받아들이는 파일 값의 보존과 사용자가 고를 수 있는 UI 범위를 별도로 처리해야 한다. General의 작은 수 지수 전환 경계 등은 도움말의 요약보다 [0022](0022-math-formatting.md)에 해시로 고정한 predicate를 기준으로 한다. 부동소수점 반올림, 음의 0, 비유한값·overflow와 원본 모든 formatter 경로의 동등성은 추가 수치 검증 대상이다.

## 증거 재현과 범위

`Ptc.Controls.Core.dll` SHA `654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977`, REA `ev_f208e991f9e1e1e33663525464eb86305e6bee7352187fed8011d74326426e64`; EquationControl·EquationEditor·Worksheet의 SHA/Evidence는 [0042](0042-edit-commit-contract.md)와 같다. ignored `artifacts/rea/result-format-decode.py` → `result-format-cil.json`/`.txt`는 파일과 선택 IL의 SHA를 검증하고 enum 상수를 정적으로 읽는다. `result-format-baml.mjs`/`.json`은 위 satellite SHA를 검사한 뒤 리소스 record를 읽는다. 원본 코드 본문·리소스·아이콘은 제품 코드나 tracked assets에 복제하지 않았다.

설치물에는 Core RealNumberFormatter와 EquationEditor `ah` 등의 숫자 표시 경로가 함께 있다. 예를 들어 Core의 zero 표현 helper 생략과 달리 `ah`의 Percent 숫자 노드는 0도 PERCENT로 감싼다. 어느 최상위 소비자가 각 경로를 선택하는지와 모든 최종 축약까지 확인하지 않은 부분을 하나의 원본 출력 규칙으로 합쳐 주장하지 않는다.

Rust의 변경 지점은 Canvas MathFormat, native formatting reader/writer, 결과 표시, 리본 dispatch, 로컬 표시 cache의 무효화다. 서식 변경을 전체 수치 재계산에 연결하면 표시만 바꾸는 원본 조작을 계산 작업으로 바꾸므로 별도로 검증해야 한다. 구현 상태와 OS 테스트 결과는 해당 변경의 검증 기록을 따른다.
