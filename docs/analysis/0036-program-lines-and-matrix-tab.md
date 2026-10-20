# 프로그램 줄 입력과 행렬 Tab

2026-10-16. Prime 12.0.0.1 설치 도움말을 정적으로 대조해 기존 구조 편집기에 입력 경로를 연결했다. GUI·스크린샷·원본 실행을 사용하지 않았다. 원본 키 상태 기계 전체를 복원했다는 의미는 아니다.

## 원본 근거

설치 `Help/en/PTC_Mathcad_Help/` 아래 파일 SHA-256을 직접 재검증했다. ignored `artifacts/rea/printing-editor-input-contract.json`에 식별자·관찰·한계를 남겼다.

| 파일 | SHA-256 | 확인된 계약 |
| --- | --- | --- |
| `program_operator.html` | `477ff512864a2355d69e206e0660b1e17b8df7c38c4ce615991b57cba96c2b28` | 프로그램은 초기 자리 표시자가 있는 블록이며 Enter로 줄을 추가한다. |
| `to_define_a_program.html` | `adc169078739f2e6b29e25abe88398a5a9ebab2b5a5660888706399b2cac9dfd` | 표현식 뒤 Enter는 다음 빈 줄, 줄 왼쪽 끝 Enter는 앞의 빈 줄을 만든다. for 블록 오른쪽 경계 밖에서 Enter하면 바깥 블록에 추가한다. 마지막 문장이 출력값이다. |
| `keyboard_shortcuts_for_editing_arrays_and_tables.html` | `077a67fee05e6ff532ec7a7176ef1a73fb91dbed2a2a53e33eab8d9a9ec54783` | Tab은 행을 따라 다음 원소로 이동하며 마지막 원소에서는 새 빈 행을 만든다. Shift+Tab은 역방향이다. |

## 독립 구현

`Editor::insert_program_line()`은 현재 문장을 찾고, 왼쪽 첫 입력점에서는 앞에, 나머지 위치에서는 뒤에 빈 문장을 추가한다. 중첩 body 안의 캐럿은 그 body에 추가하고, loop 노드의 오른쪽 경계 캐럿은 바깥 Program에 추가한다. 각 변경은 한 번의 Undo이며 기존 형제의 arena ID를 보존한다. 수식 위젯은 프로그램 안의 일반 Enter를 처리하고, 프로그램 밖의 일반 Enter와 Escape는 기존 워크시트 완료·취소 경로에 남겨 둔다. Shift+Enter의 행렬 행 추가 명령도 유지한다.

`Editor::matrix_tab()`은 자리 표시자만 찾던 기존 순환 대신 채워진 원소도 포함하여 행 우선 순서로 이동한다. 마지막 원소에서 Tab은 빈 행 하나를 만들고 첫 새 셀에 둔다. Shift+Tab은 행을 만들지 않으며 첫 원소에서는 행렬 왼쪽 경계로 이동한다. 일반 수식의 기존 Tab 동작은 유지한다. Mathcraft의 독립적인 4,096셀·전체 tree 노드 제한에 걸리면 변경하지 않는다.

빈 프로그램 줄 삭제는 **직접 Placeholder인 문장만** 제거하고 필수 마지막 한 줄을 유지하는 보수적 독립 정책이다. 채워진 문장·분기·loop를 이 동작으로 제거하지 않는다. 완전한 원본 삭제·unwrap 상태 전이는 아직 확보하지 않았으므로 원본 계약 전체와 동일하다고 주장하지 않는다.

## 검증과 남은 경계

이 작업의 편집기 단위 테스트는 **33개 통과**, 편집기 release Clippy all-targets `-D warnings`도 통과했다. 추가한 네 회귀는 앞/뒤 줄 삽입과 Undo/Redo·안정 ID, 중첩 loop body와 바깥 경계, 빈 줄 삭제와 필수 body 유지, 채워진 행렬 셀 이동과 마지막 새 행의 단일 Undo를 검사한다.

앱에는 키 이벤트 소비와 프로그램 PortableTree → JSON 왕복 → MathML → 계산값 5를 검사하는 회귀, 행렬 Tab 키와 Undo의 headless 회귀를 추가했다. 앱 전체 최종 결과는 통합 검증 기록을 기준으로 한다. 작업 도중의 검사에서 XML 접두사를 고정 문자열로 비교한 테스트 단언을 namespace/local-name 비교로 고쳤고, 병렬 변경 중의 비활성 영역 action 미완성 때문에 앱 재빌드가 일시 중단된 기록이 있다. 이 문서는 해당 중간 실행을 최종 통과로 계산하지 않는다.

원본의 전체 scope 선택·연산자 경계 기하, 모든 중첩 구조의 Enter 동작, 첫 셀 Shift+Tab 경계의 원본 런타임 비교, 전체 삭제·분기 재결합, Ctrl+J 단어 변환은 남아 있다. 이 변경으로 모든 프로그램 작성 기능을 완성한 것으로 분류하지 않는다.
