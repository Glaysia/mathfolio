# XY 첫 수식의 구조·라벨·계산 계약

2026-10-19. [정적 REA ledger](../../artifacts/rea/xy-typed-expression-next-20261019.json)는 설치된 세 assembly의 선택한 CIL 본문24개, Plots BAML과 설치 도움말의 math 조각10개를 SHA와 함께 검증한다. 원본 GUI·스크린샷·CLR/COM 실행은 없다. 원본 코드를 제품에 복제하지 않는다.

원본 PlotEquation의 첫 math는 일반 EquationControl이며 AllowExpressionsOnly가 설정된다. 두 번째 math는 SPEC_TABLE_SCALING_CELL을 쓰는 별도 ScaledEquationControl이다. 원본 writer는 두 control의 XML을 각각 직렬화한다. 따라서 첫 수식을 사칙연산 문자열로 평탄화하거나 단위 칸의 기본 UNIT 정책을 적용할 수 없다. PH도 실제 수식 구조이며 자동 배율 None과 구분한다. 상세 위치·PE SHA·MVID·CIL은 [분석 기록](../../artifacts/rea/xy-typed-expression-next-20261019.md)을 따른다.

## 독립 Rust 구현

`PlotExpression`은 기존 JSON 문자열을 그대로 보존하는 LegacyText와 검증된 PortableTree를 소유하는 Typed를 구분한다. typed JSON은 `{"tree": ...}`이며 알 수 없는 키·중복·지원하지 않는 구조를 거절한다. 루트 PH는 Typed(PH)로 남긴다. source당1024노드/64KiB, plot 합계8192노드/256KiB, legacy 문자열4096bytes는 앱 보호 제한이며 원본 한계가 아니다.

새 리본 XY는 typed PH로 시작한다. 기존 own 문자열 문서는 자동 변환하지 않는다. native 첫 math는 직접 typed로 읽으며 라벨·contextual·문자 아래첨자·숫자 lexeme·자식/연산자 구조를 비교한다. 모르는 속성·namespace·혼합 내용·지원하지 않는 서식은 Opaque 보존 경로에 남기고 지원한다고 오인하지 않는다. 문자열 표시가 같다는 이유로 native source를 인증하지 않는다.

일반 expression-only Editor가 첫 math를 편집한다. 단위 칸 Editor는 별도로 유지한다. 슬롯 삽입은 선택한 첫 math의 모델을 따르며 Typed이면 Typed PH, Legacy이면 빈 문자열을 만든다. matrix/nary 내부 명령이 외부 축 슬롯 명령보다 우선한다. 단일 필드 편집 이력과 구조 이력을 연결하며 IME preedit를 문서 이력으로 기록하지 않는다.

## label-aware 계산

[PreparedPlotQuery](../../artifacts/rea/xy-typed-sample-binding-20261019.md)는 기존 context의 label resolver와 MathML parser를 한 번 사용한다. 샘플 치환은 resolved `Symbol{name,label}`이 같은 VARIABLE 값 참조에만 적용한다. 문자 아래첨자는 별도 identity이며 같은 글자의 UNIT/FUNCTION과 구분한다. 함수 callee와 captured scope를 바꾸거나 Scope에 임시 binding을 덮어쓰지 않는다. physical Quantity를 그대로 치환하므로 단위가 텍스트 변환 과정에서 사라지지 않는다.

지원 경로는 scalar/허용 matrix·range·index·산술·허용 Call 인자 등 명시된 IR 순회다. binder/program/symbolic/logic와 함수 역할 Scale은 무차별 치환하지 않는다. 직접 평가 가능한 구조와 pointwise 치환 가능한 구조는 서로 다른 범위다. 기존500점 −10..10 fallback은 순수 undefined VARIABLE 루트와 무차원 배율 등 제한에서만 유지한다. 원본 implicit domain·binder·adaptive sampling을 복구했다는 뜻이 아니다.

첫 math의 일반 구조 보존이 모든 NodeKind 계산, 전체 contextual label 정책, rich formattingOverride, 원본 전체 navigator/IME/legend 배치나 다른 도표 종류를 완성하지 않는다. [0182 통합 검증](0182-xy-typed-expression-integration.md)과 기능 목록의 부분 구현 판정을 따른다.
