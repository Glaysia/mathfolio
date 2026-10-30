# Mathfolio 워크시트 프로토타입

**UI 개편 전 기능 체크포인트:** [현재 보존 범위](CHECKPOINT-2026-10-20.md). 다음 작업은 [현대적인 UI와 기존 단축키 안내 개선](UI-MODERNIZATION.md)이다. 기존 r222 고정 검증은 [입력 행렬 편집·조판·인쇄](analysis/0207-typed-source-matrix-window.md)에 보존한다. 아래 이름 붙은 이전 검증은 당시 이력이다.

**XY 첫 수식 후속:** [0181 계약](analysis/0181-xy-typed-expression-contract.md)·[0182 통합](analysis/0182-xy-typed-expression-integration.md)에 typed source와 일반 expression-only 편집, label/contextual/문자 아래첨자 보존·물리량 pointwise 계산·Undo/Redo·Pending 저장·cold reopen·벡터 출력을 연결한다. typed-xy-r193은 Windows/peets 각각1486검사, 양 OS98MCDX/10별도XML·58PDF 검증을 통과했다. 원본 전체 navigator·IME·sampling/domain·legend와 marker CPU 공백은 남아 있으며 완성률로 환산하지 않는다.


갱신: 2026-10-19. 지원 여부와 기능별 제한은 [통합 구현 목록](FEATURES.md)을 기준으로 한다.

이전 [XY 슬롯 키](analysis/0180-xy-slot-key-integration.md)는 Shift+Enter로 식을 앞/뒤에 추가하고, 식·단위가 모두 빈 슬롯에서 Backspace/Delete로 제거한다. Backspace는 인접 단위 칸 끝으로, Delete는 인접 식 시작으로 이동한다. 수정은 Enter에서 확정하며 문자·단위·슬롯 실행 취소 순서를 유지한다. 당시 일반 식 칸은 문자열 편집이었다. r193부터 새 도표와 native 첫 math는 일반 구조 편집기를 사용하며, 기존 own 문자열 문서는 유지한다. 모든 구조 수식 입력·legend 배치를 지원하지 않는다. Windows/peets 검증은 각1451검사와 native/vector 산출물 범위다.

이전 [구조 삭제·선택](analysis/0125-structural-deletion-integration.md)은 확인한 연산자 중심·피연산자 글자 경계의 Backspace/Delete와 괄호 쌍 제거를 연결한다. `12+34→1234`, `x+y→xy`, 유효하지 않은 소수 결합은 Scale로 남기며 Enter에서 계산을 확정한다. 일반 outer Typeover·formatted 이름과 특수 연산자의 전체 삭제는 미완료다.

빈 종이 위 원하는 위치에서 수식·텍스트를 작성하고, 수식 자체를 편집하며 위치 순서에 따라 계산하는 초기 구현이다. 수식 트리·영역 좌표·텍스트를 같은 문서에 저장한다. Mathcad 전체 동작을 재현한 제품은 아직 아니다.

현재는 GUI를 열지 않고 REA로 원본 작성·편집·계산 상태를 분석하며 headless 테스트로 구현을 연결한다. [작성 경험 기준](DESIGN.md)에 개발 방향을 기록했다. 프로그램·미적분·상징의 일부 구조 입력과 Find/Minerr 편집을 연결했으며, 지원 범위가 더 넓은 엔진 MathML 검사는 [계산 엔진 문서](ENGINE.md)의 CLI 경로로 수행한다.

## 실행과 입력

Windows 기본 실행은 release 빌드를 사용하며 실행 파일은 `target/release/mathfolio.exe`다. 과거 행 기반 실험의 `mathfolio-app.exe`와 구분한다.

```powershell
./scripts/run.ps1
./scripts/run.ps1 -Open 'worksheet.mathfolio'
```

종이에서 클릭한 뒤 바로 입력한다.

1. `a:=2`를 입력하면 변수 정의 영역이 생긴다. `:`가 정의 연산자를 만들므로 `a:2`도 같은 입력이다.
2. Enter로 아래 위치로 이동한 후 `a+3=`를 입력하면 같은 수식에 `5`가 표시된다.
3. `d:=2m`을 정의하고 아래에 `d=cm`을 입력하면 `200 cm`로 표시한다.

일반 숫자·수식·결과 단위를 수정하는 동안에는 이전 계산값을 유지한다. Enter로 편집을 마치거나 영역 밖을 클릭할 때 수정 내용을 계산에 반영한다. 수동 모드는 확정 후에도 명시 계산을 기다린다. `=`와 F5/Ctrl+F5는 명시 계산 명령이다. 프로그램 내부의 줄 추가 Enter와 행렬 Tab은 편집을 계속한다. [확정 계약과 구현](analysis/0044-edit-commit-implementation.md)을 참고한다.

[실제 열기 뒤 입력 교정](analysis/0092-committed-input-integration.md)으로 커서·키 해제에서 전체 계산 입력 준비를 제거하고 첫 행렬 Tab/Shift+Tab을 보존했다. 첫 draft의 이전 입력은 필요한 계산 명령에서 준비하며, 큰 문서의 Enter 확정 지연은 아직 남아 있다.

계산 서식의 결과 그룹에서 일반·소수·과학·공학·백분율, 후행 0, i/j 및 극형식 rad·π rad·도, 임계값을 지정한다. 이 조작은 계산값을 바꾸거나 수식을 재실행하지 않고 표시만 바꾼다. 지원되는 저장 원시 결과가 있으면 문서를 다시 연 뒤에도 원래 정밀도로 재서식한다. MCDX의 문서 기본값·영역별 서식을 읽고 저장하며 인쇄도 같은 표시를 사용한다. [구현·남은 차이](analysis/0047-result-formatting-integration.md)를 따른다.

`/`는 세로 분수, `^`는 지수, 괄호는 그룹을 만든다. Space로 부분식 선택 범위를 넓히고 Tab으로 입력 자리를 이동한다. 분수·지수 내부에서 다음 연산자를 삽입하는 자세한 규칙은 원본과 대조가 더 필요하다.

Space 범위는 시작 방향과 원래 커서를 기억하며 최대 범위 다음에는 원래 커서로 돌아온다. 선택한 복합식 위에 문자를 입력하면 그 부분을 치환한다. 붙여넣기와 확정 텍스트 이벤트는 한 번의 Undo로 되돌린다. `d:=2m=cm`처럼 정의 RHS를 평가할 수 있고 이미 평가가 있으면 `=`는 수식을 추가하지 않고 계산을 다시 요청한다. 부분 피연산자 범위와 전체 키 문맥까지 구현한 것은 아니다.

상단 수학 리본의 수학·텍스트 상자로 영역을 추가한다. Ctrl+T는 텍스트, Ctrl+Shift+M은 수식 영역이다. 텍스트 작성 후 빈 곳을 클릭하면 수식 입력으로 돌아간다. 선택한 영역의 왼쪽 가장자리 이동 영역을 드래그하거나 Esc로 영역을 선택한 뒤 방향키로 옮긴다. Shift+방향키는 5칸씩 이동하고 Enter는 다시 편집한다. 정의를 평가보다 아래로 옮기면 해당 평가의 변수 오류가 표시된다. Ctrl+Z/Y는 문서 변경과 수식·텍스트의 커서 위치를 함께 복원한다.

수식 안에서 Shift+좌우 방향키 또는 드래그로 문자 일부를 선택하여 복사·치환·삭제할 수 있다. 위·아래 방향키는 분수·행렬 등에서 가까운 입력 칸으로 이동한다. 영역 선택 상태의 Tab은 배치 순서로 다음 영역을 선택한다. 우클릭 메뉴에서 영역 복제·복사·삭제를 할 수 있다.

아래 배율 메뉴에서 종이를 확대·축소하거나 너비를 맞춘다. Ctrl+-는 이름 아래첨자이며 배율을 바꾸지 않는다. Ctrl+=는 비교의 같음, Ctrl+9/0은 ≤/≥, Ctrl+Shift+1/2/5/7은 NOT/OR/XOR/AND, Ctrl+Shift+Z는 ∞ 입력이다. F1은 단축키 안내, Ctrl+F1은 리본 그룹 숨김/복원이다. 문서 탭에서 눈금선을 표시한다. F5는 보이는 활성 영역과 마지막 보이는 계산 영역까지, Ctrl+F5는 워크시트 전체를 계산한다. [원본 단축키 계약과 제한](analysis/0057-shortcut-and-subscript-contract.md)을 따른다.

Save/Open은 `.mathfolio` v2에 수식 구조·위치·텍스트를 저장하고 다시 연다. 수식 표시를 문자열로 평탄화하거나 편집기 내부 undo 기록을 저장하지 않는다. 미완성 숫자처럼 현재 저장 검증을 통과하지 못하는 수식은 오류를 알리고 저장을 막는다. 저장하지 않은 문서를 닫거나 다른 문서를 열 때 확인한다.

계산된 수식·기호·도표의 표시 캐시도 저장하고 열기 때 계산 없이 복원한다. 표시 캐시가 없는 기존 예제/문서는 Ctrl+F5로 계산한 뒤 저장하면 다음 열기부터 결과가 복원된다. 캐시가 없다는 이유로 자동 재계산하지 않는다.

새 문서는 A4·표준 여백과 본문 격자로 시작한다. 종이 오른쪽에 초과 내용이 있으면 Page/Draft 전환으로 인쇄 범위 밖의 작성 영역을 연다. Draft에서는 여백·머리글/바닥글을 숨기고 인쇄 경계를 점선으로 표시한다. 문서 리본에서도 전환할 수 있으며 인쇄 용지 크기를 바꾸는 기능은 아니다. 저장 좌표는 본문 기준이고 화면에서만 여백을 적용한다. 리본의 미지원 명령은 비활성화되어 있다. 상세 외형 근거와 남은 차이는 [작성 경험 기준](DESIGN.md)에 기록했다.

## 공학 계산서 작성

[engineering-worksheet.mathfolio](../examples/engineering-worksheet.mathfolio)는 중력·질량·힘의 단위 계산, 행렬 determinant, 사용자 함수와 범위, XY 도표를 포함한다. 아래 명령으로 열 수 있다.

```powershell
./scripts/run.ps1 -Open 'examples/engineering-worksheet.mathfolio'
```

계산 서식 지정 탭에서 선택한 수식의 글꼴 크기(4–72pt), 소수 자릿수(0–15), 일반·소수·과학·공학 표시를 변경한다. 표시 반올림은 계산값을 바꾸지 않는다. 기본은 원본에서 확인한 3자리·일반 형식이다. 결과는 단위 분수와 위첨자를 조판하며, 변수·단위 레이블 변경은 계산 의미도 바꾸고 Undo로 복원한다.

Ctrl+Q는 식별자의 변수→단위→상수→함수→시스템→키워드→자동(None) 라벨을 순환한다. 리본의 ‘자동’도 명시 None을 원본 `labels="*"`로 저장하고 기존 제한된 라벨 추론을 적용한다. contextual로 읽은 이름은 표시 라벨과 관계없이 첫 Ctrl+Q에서 변수로 바뀐다. 이름·literal 아래첨자·문자 선택·caret을 보존하며 일반 수식의 계산 변경은 Enter로 확정한다. [원본 계약](analysis/0093-label-cycle-contract.md)의 Typeover·override 호스트·전체 wildcard 해석은 미완료다.

행렬의 셀 안에서 Shift+Enter는 행, Shift+Space는 열을 삽입한다. Tab/Shift+Tab은 채워진 셀도 행 순서로 이동하며 마지막 셀 Tab은 새 행을 추가한다. 행렬/표 탭에서 행·열 삽입과 삭제도 가능하다. 현재 셀의 시작/끝에 따른 삽입 방향을 따르고, 마지막 행/열을 삭제하면 1×1 빈 행렬로 남긴다. 프로그램 안의 Enter는 앞/뒤에 새 줄을 추가한다. 큰 행렬과 텍스트는 실제 높이에 맞춰 문서 스크롤 범위를 늘린다. 전체 원본 페이지 나눔·인쇄 pagination은 별도 미완료다.

Ctrl+2 또는 도표 탭의 XY 도표로 삽입한다. 새 도표는 원본처럼 X/Y 빈 입력칸 각 하나로 시작한다. x/y 입력에 앞에서 정의한 범위·벡터 이름이나 f(x) 같은 수식을 넣는다. x가 미정의 독립 변수일 때에는 현재 앱의 표본 정책인 −10…10, 500점을 사용한다. 원본의 전체 sampling/domain 규약은 미완료다. 표준 축 이동·확대와 모서리 크기 조절이 가능하다. 크기 조절 한 번은 Undo 한 번이며 화면 확대/이동은 문서 변경으로 기록하지 않는다.

현재 각 도표 축 식은 한 줄 source 입력이며 authored X/Y 목록의 모든 칸을 직접 편집한다. Tab은 각 Y식→단위·배율 칸을 지나 X식→단위·배율 칸으로 이동한다. 단위·배율은 구조 수식으로 편집하고 Enter 때 확정한다. 예를 들어 물리량 x의 X배율을 in, Y=x²의 Y배율을 m²로 지정할 수 있다. 자동 단위는 현재 정수 SI 기본 차원 곱이며 사용자 입력에 저장하지 않는다. 최대 16 trace, 실수·trace당 2,000점 제한이다. 배율은 유한 양수 scalar와 compatible 차원 범위만 지원하며 전체 단위 체계·affine 변환은 미완료다([0176](analysis/0176-xy-physical-units-implementation.md)). 행렬은 첫 열, 길이가 다른 벡터는 공통 구간을 사용한다. 비유한 값·복소수·미지원 입력은 오류를 표시한다. 기본 축·label/marker가 없는 선 그래프의 식/서식·지원 finite 물리량 곡선과 residual unit은 MCDX로 저장하고 재계산 없이 다시 연다([0147](analysis/0147-native-xy-integration.md)). 미지원 native 형식은 raw로 보존하며 해당 기능을 편집해 내보내려 하면 명시적으로 거절한다. inline 축 입력은 Enter/편집 종료 때 확정하며 수동 모드에는 이전 곡선과 Pending을 유지한다. 한글 조합 중 Enter는 식 확정으로 처리하지 않는다([0153](analysis/0153-inline-xy-commit-integration.md)). 공유 축 목록·불균등 native import·공유 primary 입력/설정 편집·계산·저장은 연결했다([0156](analysis/0156-shared-xy-axis-integration.md)). 공유 목록은 pair로 펼쳐 편집하지 않는다. 슬롯 모델 API와 빈 PH의 MCDX 저장은 연결했으며([0160](analysis/0160-xy-slot-render-integration.md)) Shift+Enter 새 슬롯·빈 Backspace/Delete·방향별 포커스·혼합 Undo를 실제 키로 연결했다([0180](analysis/0180-xy-slot-key-integration.md)). 각 곡선의 정상/오류/계산 대기를 따로 보관하므로 한 곡선 오류가 정상 sibling을 지우지 않는다. 자체 저장은 오류 진단을 보관하며, native 저장은 오류 난 옛 curve를 제거한 Pending으로 내보낸다([0167](analysis/0167-per-trace-plot-integration.md)). legend 위치/폭은 source로 보존하며 실제 좌우 배치는 미완료다. native graph-size와 외곽 요청/실측 크기를 별도로 보존하며, 크기 조절 Undo는 정밀 source 전체를 복원한다([0172](analysis/0172-xy-geometry-slot-integration.md)). 전체 지원 범위는 기능 목록을 따른다.

머리글·바닥글은 문서 리본에서 기본 텍스트·위치와 페이지/전체 페이지·파일명·경로·저장 날짜 필드를 편집하고 저장할 수 있다. 저장 날짜는 실제 열기/저장 날짜를 캐시해 표시한다. 첫 페이지 별도 서식·rich text·완전한 pagination은 미구현이며 기본 인쇄/PDF는 아래 범위로 지원한다.

다중 trace의 축 레이블·명시 범위·선과 표식 서식을 연결했지만 일부 선 스타일은 근사 구현이다. 실제 Prime 축 입력 UI와 모든 단위/배열 형식을 재현하지는 않는다.

## MCDX와 지원 경계

MCDX 열기는 일부 기본 수식의 구조 편집과 텍스트 표시를 지원한다. 명시 라벨, 괄호, 분수·지수, 정의·평가와 표시 단위를 보존하며, 표현하지 못하는 영역은 편집 불가로 표시한다. 원본 ZIP 바이트도 v2 문서 안에 그대로 남긴다. 편집기가 읽지 못한 native math도 지원 엔진으로 계산하며, 미지원 영역 하나 때문에 독립된 모든 영역 계산을 중단하지 않는다. 실패한 정의는 종속 계산으로 오류를 전파하고 이전 캐시를 새 계산처럼 보여주지 않는다. 상징 결과와 풀이 블록 요약에도 계산된 값을 표시한다.

현재 저장 형식은 `.mathfolio` v2이며 Export MCDX와 CLI `sheet-export`가 새 MCDX로 편집 내용을 내보낸다. 원본 parts·관계를 보존하고 결과 캐시를 Pending으로 무효화한다. 생성 구조의 XSD 검사는 통과했지만 원본 앱에서 다시 열기는 검증하지 않았다. v1 행 기반 실험 문서와 형식이 다르다. 원본 MCDX 파일은 변경하지 않는다. 가져오기 원본은 현재 16 MiB로 제한한다.

## 인쇄와 페이지 설정

Ctrl+P 또는 파일 메뉴의 인쇄에서 프린터·매수·전체/현재/연속 범위/선택 영역이 걸친 페이지를 선택한다. `1-3`은 가능하고 `1,3`은 받지 않는다. 현재 결과를 사용하며 인쇄가 강제 계산을 실행하지 않는다. 종이 밖 Draft 영역은 출력하지 않고, 종이 안 오른쪽 여백까지 걸친 표는 종이 경계에서 자른다. PDF는 경로·글자·부분 글꼴을 사용해 확대와 검색·선택이 가능하다. Windows 물리 인쇄는 기존 래스터 fallback, Linux CUPS는 벡터 PDF를 사용한다. 실제 프린터와 드라이버 출력은 아직 검증하지 않았다.

가져온 MCDX의 지원되는 텍스트/수식 배경색·텍스트 서식과 Spec Table은 저장된 원본 표현으로 표시·인쇄한다. 가져온 표의 이름·단위·데이터 셀을 클릭해 구조 수식으로 편집한다. Enter·셀 이동·영역 이탈에 확정하며 입력 중에는 계산하지 않는다. Tab/Shift+Tab은 다음/이전 셀, 마지막 셀 Tab은 빈 행 추가다. 여러 값의 열은 단위가 있는 열벡터, 한 값은 스칼라로 후속 수식에 연결한다. 행렬/표 리본에서 새 표와 행/열 삽입·삭제를 사용한다. 셀 간 드래그는 사각 범위를 선택하며 Copy/Cut/Backspace/Delete와 크기를 유지하는 Clear Cells를 지원한다. 본문 Copy는 행렬, 이름/단위 행을 포함한 Copy는 새 표다. 수식 Paste는 활성 셀의 초안을 수정하며 행렬을 인접 셀에 임의로 나누지 않는다. 원본 OS clipboard binary 형식과 전체 셀 전이·중첩 배열 계산은 미완료다. [분기 교정](analysis/0064-table-clipboard-application.md)을 따른다. 지원 native Paragraph/Run에서는 선택한 글자의 굵게·기울임·크기·색상과 Run 내 확정 치환을 원문에 보존한다. IME는 확정 전 화면 draft다. 미지원 문단 geometry는 readonly 서식을 유지하며 새 rich Text·문단/cross-Run 내용 편집·전체 rich clipboard는 미완료다. [원문 통합](analysis/0194-rich-text-source-integration.md)을 따른다. 재계산 뒤 V·Ω·W 등이 기본 단위 조합으로 바뀌던 오류는 원본 SI 대표 단위 선택을 적용했다. 전체 단위계·복합 단위 분해는 미완료다.

문서 리본의 크기·여백에서 A1–A5/Letter/Legal/Ledger, 세로·가로, 프리셋 또는 상하좌우 여백을 설정한다. 자체 문서 저장·열기와 MCDX export에 보존한다. 머리글/바닥글의 저장 날짜는 열기/저장 시 캐시한 실제 날짜를 표시하고 인쇄에도 반영한다. [인쇄 구현](analysis/0035-printing-implementation.md)과 [날짜 경계](analysis/0038-saved-date-fields.md)를 참고한다.

선택한 계산 영역은 계산 리본 또는 우클릭의 영역 비활성화로 제외한다. 편집은 가능하며 마지막 결과를 흐리게 보존한다. 비활성 정의는 후속 계산에 바인딩을 제공하지 않는다. 재활성화하면 다시 계산 대상이 된다. Area 상위 비활성화는 별도 미완료다.

기본 산술·단위·정의·함수·행렬·인덱스·범위 입력과 구조 클립보드를 연결했다. 프로그램·calculus·상징의 구조 입력·표시와 Find/Minerr의 제한된 편집 경로가 연결됐다. 프로그램 Enter 줄 추가와 빈 줄 삭제는 연결했지만 전체 제어문 저작·삭제/unwrap, 전체 상징 명령, Prime 인라인 풀이 블록은 미완료다. MathML 엔진 지원과 편집기 지원은 별도 범위다. 완전한 키 전이·실제 OS 한글 IME·모든 원본 영역의 출력은 남아 있다. STIX 수학 글꼴로 기본 크기·레이블 스타일을 적용하지만 글리프·일부 간격은 근사 구현이다.

## 검증

창을 띄우지 않는 테스트에서 입력·이동·단위·저장 수명 주기와 확대 클릭 좌표, 문자 선택·드래그, 수직 이동, Undo 커서 복원, IME 조합/확정 상태를 확인한다. 현재 GUI 조작이나 스크린샷 검사는 하지 않는다. 실제 OS IME·후보창과 전체 키 전이의 실기 검증은 미완료다. 원본 38개 최소 사례 비교의 정확한 범위는 [호환성 검증](CONFORMANCE.md)을 참고한다.

## 조사로 확인한 원본과의 차이 — 2026-10-16

문서를 열면 지원되는 MCDX 저장 결과와 자체 문서의 수식·기호·도표 표현을 계산 없이 표시한다. 캐시가 없거나 불일치하면 Pending으로 남겨 명시 계산을 기다린다. 수동 편집 뒤에는 이전 결과를 유지하고 영향받은 영역만 흐리게 표시한다. F5는 보이는 활성 영역을 먼저 강제 갱신한 뒤 보이는 마지막 계산 영역까지 Pending 구간을 처리하며, Ctrl+F5는 전체를 강제 계산한다. [REA 및 구현 기록](analysis/0032-calculation-commands-and-saved-results.md)을 참고한다. 계산은 백그라운드에서 실행한다. 계산 중 **모든 계산 중지**를 누르면 수동 모드로 바뀌며 미완료 영역과 이전 결과를 유지한다. 자동 계산을 다시 켜면 남은 구간을 재개한다. 자동 계산 체크 해제는 진행 중 작업의 강제 중지와 구분한다. [중지·재개 구현 범위](analysis/0041-background-calculation-ui.md)를 참고한다. 전체 native cache 형식·정밀 의존성·Area 전파·원본 queue 게시 시점·외부 IO 중단은 미완료다.

큰 기호 결과는 원본에서 오른쪽 resize handle로 영역 폭을 늘려 확인한다. 이 작성 동작은 아직 미구현이다. Page/Draft와는 별도이며 자체 결과 모달을 추가하지 않는다. 전체 선택 전이와 상징 명령 쌓기/숨김도 남아 있다. 더 넓은 기능 목록은 [통합 목록](FEATURES.md)의 세부 행을 따른다.

일반 수식의 배열 원소 정의도 구조로 저장한다. 한/두 index, 범위 변수의 행렬 정의, 기존 배열·행벡터 갱신/확장, ORIGIN, 단위·복소수를 지원한다. 문자열 원소·자기참조 범위 recurrence·일반 행벡터 단일-index 조회와 fill metadata 없는 혼합 단위 확장은 미완료다. [원본 계약과 경계](analysis/0063-indexed-definition-contract.md)를 따른다.

일반 행렬의 여러 셀은 포인터로 직사각형 선택해 Copy/Cut/Delete/Backspace 또는 리본의 행/열 삭제·셀 내용 지우기를 사용한다. Delete와 크기를 유지하는 Clear는 구분한다. 범위 Paste와 일반 수식 입력은 확인한 원본 모드대로 거절한다. 행렬 변경도 Enter/이탈로 확정한다. [입력·저장 검사와 남은 차이](analysis/0068-matrix-workflow-integration.md)를 따른다.

수학의 연산자 갤러리에서 팩토리얼·전치·복소수 켤레·절댓값/행렬식·노름·행 추출·열 추출을 삽입한다. 직접 키는 `!`, Ctrl+Shift+T, Ctrl+Shift+(, `|`, Ctrl+Shift+|, Ctrl+Shift+R/C다. Norm은 내부적으로 `determinant`라는 원본 명칭을 쓰며 새 `norm` XML 태그를 만들지 않는다. [단축키·조판](analysis/0070-structural-array-operators-input.md)과 [타입/저장 경계](analysis/0072-structural-operators-integration.md)를 따른다. Ctrl+Shift+C는 열 연산자이고, 실제 Copy 이벤트와 일반 Ctrl+C는 복사를 유지한다.

전용 전역 정의는 Ctrl+Shift+~ 또는 리본, 벡터화는 Ctrl+Shift+^, 벡터 외적은 Ctrl+8로 입력한다. 전역 정의는 위쪽 수식에도 적용하고 벡터화는 화살표 아래 식의 연산자·함수에 원소별 계산을 적용한다. 표의 헤더·단위·본문 셀은 원본처럼 표현식 전용으로 정의 명령을 거절한다. [지원 범위](analysis/0076-global-vector-integration.md)를 따른다.

입력 %는 해당 식에 0.01을 곱하며 결과 Percent 서식과 별개다. °는 π/180배 Degree 연산자다. Ctrl+Shift+P는 극형식 ∠를 만든다. 예를 들어 2∠90°는 2i다. 현재 Polar의 물리 단위는 완성된 phasor 뒤에 곱한다. [원본 계약과 한계](analysis/0080-scalar-notation-integration.md)를 따른다.

행·열 연산자를 일반 정의·전역 정의·프로그램 내부 대입의 왼쪽에 둘 수 있다. Enter 전에는 초안이며 확정 후 뒤쪽 계산을 갱신한다. [저장·계산 경계](analysis/0083-array-slice-integration.md)를 따른다.

프로그램 직접 statement에 `f(x)`를 입력한 뒤 `{`를 누르면 `f(x) ← □`를 만든다. 함수 body와 다음 줄 호출을 입력할 수 있다. [지역 함수와 확정 경계](analysis/0087-local-function-integration.md)를 따른다.

수식의 정확한 소문자 `while`, `for`, `return`, `break`, `continue`, `if`, `try`를 입력한 뒤 Ctrl+J로 구조로 바꾼다. 원본 라벨·formatted 이름 조건과 한 Undo를 유지한다. 인접 분기 단어와 지원 경계는 아래 설명을 따른다.

프로그램 리본의 if는 빈 자리에서 test/body 한 쌍을 만든다. IF 오른쪽 또는 미완성 IF 바로 다음 빈 Program 문장에서 else if/also if/else를 삽입한다. Ctrl+J if를 지원하며, 인접 Program 문장의 else/elseif/alsoif도 같은 수리 경로로 변환한다. middle operator 단어를 클릭한 뒤 else if/also if로 전체 모드를 전환할 수 있다. 결과는 Enter로 확정한다. 일반 populated 분할·병합은 미완료다. [지원 범위](analysis/0101-conditional-branch-integration.md)를 따른다.

빈 위치의 Try/on error 리본 또는 정확한 전체 소문자 `try`의 Ctrl+J로 두 본문을 삽입한다. 첫 본문 성공 시 그 값, 지원하는 계산 오류 시 두 번째 본문을 실행한다. `on error` 헤더 삭제는 두 본문을 순서대로 합치며 최초 빈 입력칸 Backspace는 Try를 제거한다. 결과는 Enter로 확정한다. [지원 경계](analysis/0105-try-control-integration.md)를 따른다.

프로그램에서 식 범위를 Space 등으로 선택한 뒤 while/for/if/try 리본을 누르면 기존 식을 본문으로 보존한다. 값의 글자 전체를 선택한 Normal 입력은 빈 연산자로 교체하는 별도 경로다. Return Scope의 방향 제한·커서와 지원하지 않는 부분 선택 경계는 [0109](analysis/0109-program-insertion-integration.md)를 따른다. 초안 결과는 기존대로 평가 경계 Enter에서 확정한다.

숫자와 단위를 붙여 입력하면 Scale이 생기며 Ctrl+Shift+U로 직접 삽입할 수도 있다. 활성 수식에서만 ∘를 표시한다. 오른쪽 이름이 함수면 왼쪽을 한 인자로 호출하고 값/단위면 곱한다. 명시적 *는 그대로다. 지원 범위와 Ctrl+J 본문 보존·Enter 확정은 [0113](analysis/0113-implicit-scale-integration.md)을 따른다.

숫자 뒤 `(`는 새 괄호 피연산자, 이름 뒤 `(`는 함수 호출이다. 지수·분모 안의 입력은 그 안에 남는다. 바깥 연산자/단위를 입력하려면 ArrowRight로 나간다. `)`도 현재 구조에 따라 닫기 또는 괄호 삽입을 한다. [지원 분기와 예](analysis/0117-parenthesis-integration.md)를 따른다.

일반 연산자는 Normal 문자 범위를 사용한다. `1234` 중간의 `+`는 `12+34`, `1[23]4` 선택 후 `+`는 `1+4`다. 문자 전체 선택은 빈 연산자로 교체하며 Scope는 기존 식을 보존한다. Ctrl+-로 literal suffix 끝을 빠져나온 후 연산자를 이어 입력할 수 있다. [커서·선택의 지원 범위](analysis/0121-infix-entry-integration.md)를 따른다.
