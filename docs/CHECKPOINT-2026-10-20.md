# UI 개편 전 기능 체크포인트

2026-10-20. 현재 누적 구현을 커밋·태그로 보존하고 다음 작업을 [현대적인 UI 개선](UI-MODERNIZATION.md)으로 전환한다. 기존 단축키는 유지한다. 사용자의 최신 평가는80%이며, 아래 검사는 개별 지원 범위의 회귀 확인이다.

보존 태그: `checkpoint/pre-ui-modernization-2026-10-20`.

## 보존하는 구현

수식·문자·단위·행렬·표·프로그램 편집, Enter 확정과 Undo/Redo, 계산 상태와 저장 결과 복원, native 문서 저장·내보내기, Area·페이지·XY 도표·벡터 PDF 등 지난 커밋 이후의 누적 소스를 보존한다. 기능별 지원 범위와 미구현 항목은 [통합 목록](FEATURES.md)에 기록한다.

마지막 행렬 후속 변경에는 source 표시 범위가 있는 행렬의 제한된 행·열 삽입·삭제, 전체 셀과 표시 범위의 분리, 부분 복사, 실제 앱 명령·Enter·Undo/Redo·저장·cold reopen 검사가 포함된다. 원본의 위치별 표시 범위 전이와 전체 크기 생성 경로를 따르며, 새 목표 caret가 숨거나 삭제 후 진입할 복합식의 원본 navigator가 미확정인 경우는 변경을 거절한다.

앱 검사에는4×4 행렬의 행·열 삽입/삭제, metre 행렬의 새 행 입력과 cm 결과 변환, 마지막 Tab의12→13행 전체 크기 생성이 포함된다. 원본 Cut의 복사 후 삭제 순서를 유지한다. source 직접 resize·전체 nested navigator 등은 남아 있다.

## 체크아웃 재현성

기본 빌드·검사는 Git에 보존하는 소스와 참조 fixture로 수행한다. 행렬 기본 서식 검사는 추적되는 presentation XML을 작은 테스트 패키지에 넣어 실행한다. 전체 개인 LCC 문서를 쓰는 원본 검사는 명시적인 ignored 검사로 유지하고 컴파일 시 개인 파일을 포함하지 않는다.

실행 파일·빌드 산출물·로컬 REA 분석 자료·Python 캐시·개인 code-workspace는 Git에서 제외한다. 기존 로컬 자료는 보존한다.

## 검증 기록

Git index에서 내보낸704파일의 소스로 검사했다. 두 환경 모두 개인 REA artifacts가 없는 체크아웃을 사용했다. 마지막 문서 갱신 전후에 프로그램 소스가 동일함을 확인했다. GUI·스크린샷·원본 Mathcad 런타임은 실행하지 않았다.

| 검사 | Windows | peets Kubuntu |
| --- | --- | --- |
| workspace와 입력 backend 테스트 | 2177 통과 | 2178 통과 |
| 선택 실행용 ignored 검사 | 105 | 103 |
| fmt·workspace/backend Clippy | 통과 | 통과 |
| workspace 빌드 | 통과 | 통과 |
| 기존 참조 표본38건 | 통과 | 통과 |
| release 빌드 | 통과 | 이번 검사에서 미실행 |

Windows release는 26,221,568바이트이며 SHA256은 `88462d5209258a855454af673866e21aef9605bfd4959f3b10e81d08958db70a`다. 실행 파일을 켜지 않고 빌드만 했다. 상세 로그는 로컬 `artifacts/rea/pre-ui-checkpoint-20261020/`에 보존한다.

이전 [r222 검증](analysis/0207-typed-source-matrix-window.md)은 별도 고정 소스로 Windows2164·peets2165 정상 검사와 MCDX20·own12·PDF12·선택 XML XSD120건을 통과했다. 해당 packet은 당시 기록이며 이번 커밋 검사로 재생성했다고 부르지 않는다.

## 다음 작업

[UI 개선과 단축키 안내](UI-MODERNIZATION.md)를 먼저 진행한다. [CLI·MCP·skill](AGENT-INTEGRATION.md)은 계획만 남긴다. Obsidian과 VS Code 확장은 별도 프로젝트 방침을 유지한다.
