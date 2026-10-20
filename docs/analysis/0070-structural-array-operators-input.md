# 구조 배열 연산자의 입력·그림·히트 영역

2026-10-16. 원본 descriptor와 편집·MathML 계약은
[0069](0069-structural-array-operators-contract.md), 계산 지원과 한계는
[0071](0071-structural-array-operators-calculation.md)를 따른다. 이 단계는
`formula.rs`와 별도 headless 테스트 파일만 변경했다. 원본 앱 실행,
GUI·스크린샷, CLR·COM은 사용하지 않았다. REA metadata/raw IL과 설치 도움말을
읽고 독립 Rust AST renderer와 입력 경로를 연결했다.

## 원본 표기와 독립 배치

추가한 AST는 `Factorial`, `Transpose`, `Conjugate`, `Determinant`, `Row`,
`Column` 여섯 개다. 기존 `Absolute`와 합쳐 원본의 일곱 명령을 표현한다.
별도 `Norm` AST나 네이티브 `<norm/>` 태그를 만들지 않았다.

| 사용자 명령 | 입력 | 저장 AST | 그림 |
| --- | --- | --- | --- |
| Factorial | `!` | Factorial | 피연산자 뒤 `!` |
| Matrix Transpose | Ctrl+Shift+T | Transpose | 우상첨자 `T` |
| Complex Conjugate | Ctrl+Shift+( | Conjugate | 피연산자 전체의 윗줄 |
| Absolute Value/Determinant | `\|` | Absolute | 양쪽 단선 세로 fence |
| Norm | Ctrl+Shift+\| | Determinant | 양쪽 이중 세로 fence |
| Matrix Row | Ctrl+Shift+R | Row | 오른쪽 위 index, 위·아래 angle fence |
| Matrix Column | Ctrl+Shift+C | Column | 오른쪽 위 index, 좌·우 `〈 〉` fence |

Norm의 내부 descriptor 이름은 DETERMINANT다. descriptor 문자열 `'||'`만
보고 단선이나 이중선을 결정하지 않았다. `VisualFactory.CreateDeterminate`
`0x0600069a` → `VisualLinearOutfixNode.CreateDeterminate` `0x06000895`가 양쪽에
`CreateDoubleVerticalLine`을 사용한다. 기존 Absolute의 factory는 단선이다.
원본 artifact SHA, MVID, REA evidence ID와 raw IL identity는 0069에 보존했다.

Row/Column 배치도 일반 원소 아래첨자와 다르다. 원본
`CreateMatrixRowNode/ColNode` `0x06000688/689`는 공통
`VisualMatrixSuperscriptBinaryNode`를 사용한다. `ArrangeOverride`
`0x06000b18`은 base를 아래로 밀고 index를 base의 오른쪽, y=0에 배치한다.
Row content `0x0600068a`는 `CreateVerticalBrackets`, Column content
`0x0600068b`는 `CreateHorizontalBrackets`를 만든다.

Column의 `0x06000c9b`는 LeftAngleBracket/RightAngleBracket으로 index를 감싼다.
Row의 `0x06000c9c`는 horizontal=false 및 LowerAngleBracket/UpperAngleBracket을
사용한다. `HorizontalFence.CreateLower/UpperAngleBracket` `0x060004e4/e5`는
각각 U+27E8/U+27E9와 reverseCoordinates=true를 넘기며
`DrawRotatedText` `0x060004e1`은 -90도로 회전한다.
`VisualLinearOutfixNode.ArrangeStaticLayout` `0x0600089f`의 수직 분기는
right/Upper fence → index → left/Lower fence 순서다. 따라서 Row index의
위에는 ∧ 방향, 아래에는 ∨ 방향의 angle fence가 놓인다. 임의의 대괄호나
가로 angle 쌍으로 치환하지 않았다.

독립 renderer는 이 표기·상대 위치를 구현하며 STIX 대체 글꼴과 자체 vector
선을 사용한다. PTC 원본 font/resource 또는 구현 코드를 제품에 복사하지 않았다.
script 비율, fence 간격, 윗줄 여백은 기존 독립 renderer의 배치 정책이다.
원본 pixel geometry 전체가 같다는 주장은 하지 않는다.

원본 descriptor가 금지하는 nested base 조합은 같은 binding power라도
괄호가 필요하다. Transpose(Row/Column), Row/Column(Transpose/Row/Column)은
raw imported AST에서도 보이는 괄호를 추가하되 source tree를 변경하지 않는다.
Core 편집 명령이 이미 Group을 만든 경우에는 괄호를 중복하지 않는다.
Factorial 및 다른 script base는 확인된 binding power에 따라 기존 괄호 정책을
사용한다. named 함수 Call은 새 operator로 바꾸지 않는다.

## 입력·클립보드·결과 경로

`!`는 postfix 삽입이고 Ctrl+Shift+T/R/C 및 Ctrl+Shift+9(미국 배열의 `(`)는
해당 Core 명령을 호출한다. Norm의 원본 OemPipe 단축키는 egui의 Pipe와
Backslash key 표현을 모두 받는다. 기존 Ctrl+- literal subscript,
Ctrl+9 비교 명령, Ctrl+Shift+1 논리 명령과 modifier를 구별한다.
함수 이름 `factorial`, `transpose`, `conj`, `matrow`, `matcol`, `norm`에
괄호를 입력한 Call은 그대로 Call이다. 기존 `abs(` 입력 별칭은 이 단계에서
다른 함수 정책으로 변경하지 않았다.

실제 backend 경로도 검사했다. upstream `egui-winit 0.36.2`의
`is_copy_command`는 command+C에서 Shift를 무시하고, key 처리부는 Copy 이벤트를
보낸 뒤 반환한다. 따라서 Ctrl+Shift+C의 Key 이벤트만 headless로 시험하면
실제 창에서 Matrix Column을 입력할 수 없다. 그러나 Copy와 현재 modifier만으로
원래 C 키였다고 역추정할 수도 없다. Windows Ctrl+Shift+Insert, 전용 Copy 키,
메뉴 Copy도 같은 이벤트를 만들 수 있기 때문이다.

초기 Copy+modifier 우회는 제거했다. parent는 공개 OSS backend를
`vendor/egui-winit`에 해당 MIT/Apache 라이선스와 함께 보존하고 Cargo patch로
연결한다. C를 Copy로 바꾸는 조건에서 Shift와 Alt를 제외해 Ctrl+Shift+C를
실제 Key 이벤트로 전달한다. Ctrl+C, Ctrl+Insert, 전용 Copy 키의 기존 clipboard
경로는 유지한다. formula는 정확한 Key C+Ctrl+Shift에서만 Column을 삽입하며
Copy 이벤트는 modifier와 관계없이 복사로 처리한다. 별도 키 alias나 손실된
키 정체를 추측하는 helper는 없다. backend predicate 자체의 회귀는 parent의
통합 범위이고, formula는 `ModifiersChanged`+Key C와
`ModifiersChanged`+Copy를 구분하여 검증한다.

MatrixCells 범위 상태에서는 새 수식 명령을 Core 계약대로 no-op 처리한다.
원본에 없는 선택 범위 분배, tile, 자동 확장, 추가 shortcut은 없다. rejected
명령은 arena revision이 변하지 않으므로 dirty response나 계산 요청을 만들지
않는다. 일반 명령의 keystroke는 계속 draft이며 Enter의 parent 확정 경계를
바꾸지 않았다.

장식 `!`, `T`, 윗줄, fence는 paint primitive이며 새로운 편집 노드를 만들지
않는다. 피연산자와 Row/Column index의 hit는 실제 source NodeId에 연결된다.
Readonly 측정·그림은 AST나 선택을 바꾸지 않고 keyboard input을 소비하지 않는다.
측정과 egui 할당 크기는 실제 pixel snap 범위 내에서 일치한다.

계산 결과의 구조 표기에는 parent가 추가한 `DisplayMath::Overline`,
`Bars(child,double)`, `AngleFence(child,vertical)`를 같은 geometry helper로
그린다. Row/Column 결과는 Power(base, AngleFence(index))의 구조이며 결과의
장식을 편집 가능한 source index와 혼동하지 않는다. 저장된 결과 codec과
native symbolic 변환의 별도 검증은 parent integration 범위다.

## 검증

`formula/array_operator_tests.rs`의 최종 10개 연산자 입력 회귀는 다음을 검사한다.

- Factorial `5! = 120`, `1+2! = 3`, 한 입력 명령 Undo.
- Transpose/Row/Column/Conjugate/Norm 단축키의 전용 AST와 operand 보존.
- 정확한 Ctrl+Shift+C Key → Column, clipboard write 없음.
- Shift가 눌린 Copy 이벤트는 원래의 복사를 수행하고 식·Undo를 바꾸지 않음.
- 여섯 새 AST와 기존 Absolute의 native MathML 왕복, readonly input 비소비,
  크기 측정, finite layout, 실제 source operand hit 보존.
- Column의 좌우 angle, Row의 위아래 angle 방향 및 index text hit.
- native parse 후 평가: transpose `[1,3;2,4]`, 두 번째 행 `[3,4]`, 두 번째
  열 `[2;4]`, 복소수 conjugate `2−3j`, 단위 없는 두 bar operator의 예제 결과.
- 원본 금지 nested script의 visible grouping과 AST 불변.
- named Call과 operator 입력 경로 분리.
- MatrixCells 상태의 모든 신규 명령 no-op, dirty·undo·계산 요청 없음.

첫 연산자 통합의 formula 전체 회귀는 66 passed, 1 ignored(CPU benchmark)였다.
그 뒤 backend 교정이 반영된 Cargo.lock을 사용한 `--locked --offline` 검사에서
최종 연산자 10개가 모두 통과했고, app clippy
`--all-targets --no-deps --locked --offline -- -D warnings`도 통과했다. Parent의 실제
worksheet 저작·저장·native export·재열기, 기존 1015/1016 문서 검사와 양 OS
전체 검사 결과는 최종 integration snapshot에 기록한다. 원본 앱으로 생성물을
다시 열어 본 것이 아니며 모든 원본 caret/navigation 상태나 native clipboard
binary 상호운용까지 완성했다는 의미는 아니다.
