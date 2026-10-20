# 문자열 수식 값의 원본 계약과 독립 구현

2026-10-19. 문자열은 텍스트 영역과 다른 계산 값이다. [편집 ledger](../../artifacts/rea/string-value-next-contract-20261019.md)는 설치 CIL74methods/29raw body와16XML 예제를, [계산 ledger](../../artifacts/rea/string-engine-next-contract-20261019.md)는23개 설치 IL 본문과6도움말/43source 예를 구분한다. 이 수치는 구현 기능 수가 아니다. GUI·스크린샷·원본 CLR/COM 실행 없이 바이트·토큰·SHA를 연결한다.

## 입력과 저장

원본 STRING은 별도 descriptor와 StringValueNodeInfo를 사용한다. 내부 값 캐럿에서는 EquationPresenter063d가 기본 GestureManager를 비활성화한다. 따라서 내부의 `+`, `=`, 공백, ASCII 따옴표는 연산자나 닫기 명령이 아니라 본문 문자다. 외곽에서는 기본 gesture를 다시 사용한다. 마지막 문자를 삭제해도 선호 descriptor를 거쳐 빈 STRING을 유지한다. STRING의 UTF-16 값 범위와 양쪽 외곽을 구분한다.

VisualStringNode는 저장된 본문 밖에 곡선 따옴표를 그린다. 문자열 결과도 formatter가 STRING descriptor를 생성해 같은 view로 연결된다. source writer는 `<ml:str xml:space="preserve">본문</ml:str>`, native 결과 Core0177은 `<ml:str>`와 WriteString(value)를 쓰며 source의 xml:space나 시각적 따옴표를 추가하지 않는다.

Rust는 `NodeKind::StringLiteral(String)`, `Expression::StringLiteral(String)`, `Value::String { value }`로 구조·계산·저장된 값을 구분한다. legacy JSON의 기존 variant와 source 의미는 유지한다. structural bridge는 문자 내용을 parser로 재해석하지 않고 unknown 속성·구조·comment/PI를 거절한다. XML normalization으로 CR이 바뀌지 않도록 writer가 character reference를 사용한다.

빈 문자열과 PH는 다르다. 원본 regex의 C-category 제외에 맞춘 키 입력은 현재 Unicode16 테이블과 BMP 문자 범위에서 지원한다. raw portable/native는 XML-valid supplementary 문자를 보존하며 UTF-16 범위를 쓰되, Rust String에서 서로게이트 반쪽을 저장할 수 없어 이동·삭제는 완전한 scalar 경계다. 도움말1024자 제한의 정확한 검사 위치/단위는 미복원이다. 현재16KiB atom 예산을 원본 한계라고 주장하지 않는다. 외곽 따옴표 삭제·전체 선택 복구·원본 clipboard와 모든 형식 문자 입력은 미완료다.

## 계산과 문서 설정

문자열 리터럴·정의·함수 인자/반환·지원 프로그램의 Value 흐름을 연결한다. `concat`, UTF-16 code-unit `strlen`, ORIGIN 옵션을 반영하는 `substr`와 제한된 `search`, 유한 invariant decimal의 `str2num`을 지원한다. substr은 요청 길이를 남은 길이까지 줄이며 서로게이트를 가르는 범위는 명시적 미지원이다. search는 원본 .NET collation 전체를 복원하지 않았으므로 ASCII에 제한한다. num2str의 NumVal.ToString/culture 규약·혼합 문자열 행렬·전체 vector/string 변환을 임의로 구현하지 않는다. 함수의 원본 오류 분류·우선순위 전체 일치도 미완료다. 계산 String1MiB는 독립 앱 예산이다.

calculation10.xsd의 `strings-use-origin` 기본값은 false다. package의 고유 ContentType Override로 실제 settings part를 읽고 boolean을 검사하며 파일명 고정 가정을 하지 않는다. source archive SHA에 묶인 준비 cache에서 읽은 옵션은 direct calculator와 background job 모두에 전달한다. 옵션이 바뀌면 기존 globals/캡처된 함수 scope를 무효화한다. malformed/누락된 active settings를 false로 오인하지 않는다. 옵션을 읽을 수 없는 preserved archive에서는 숫자·strlen·concat 등을 유지하며 origin-sensitive substr/search만 미지원으로 남긴다.

이 설정의 새 UI를 추가한 단계는 아니다. 기존 native part를 own/native 저장에 보존하고 현재 계산에 반영하는 경로다. [0185 통합 검증](0185-string-value-integration.md)은 실제 키·확정·저장·재열기·worker·출력 범위를 기록한다. 원본 전체 문자열 기능이나40% 달성을 뜻하지 않는다.
