# 행렬 삽입과 가져온 Area의 문서 흐름

2026-10-19. 사용자 평가15%에서40% 목표로 계속 개발하는 중간 묶음이다. 이 기능 수·검사 수를 제품 완료율로 환산하지 않는다. GUI·스크린샷·원본 Mathcad/CLR/COM 실행 없이 설치 바이트의 REA/CIL, 저장 스키마, CPU 기반 앱 입력과 독립 PDF/XML 검사를 사용한다.

## 연결한 흐름

- [0131](0131-matrix-insertion-contract.md): Ctrl+M·빈 `[`·리본 무인자 삽입의 기본1×1, 선택값 wrapping과 인접 Scale, 실제 키로2×2 작성/Enter/Undo/native 저장.
- [0132](0132-area-contract.md)·[0133](0133-area-calculation.md): 가져온 Area의 로컬 자식 좌표와 소유권, 부모 위치에서 자식 블록 전체를 계산하는 순서, 접혀도 계산, 일괄 disable 방향. 자식을 평면 Canvas 영역으로 복제하지 않는다.
- [0134](0134-area-child-editing.md)·[0135](0135-area-saved-presentation.md): 자식 수식은 draft로 수정하고 Enter/포커스 경계에서 원래 XML의 해당 식만 반영한다. 이전 결과를 유지하고 자체 저장 결과를 실행 없이 복원한다. source/ref/ID가 다른 native cache는 재사용하지 않는다.
- [0136](0136-area-printing-contract.md): 앱 PrintSnapshot에 자식 수식·결과·지원 rich text/표를 연결하여 vector PDF로 출력한다. 접힌 자식/아이콘을 출력하지 않으며 숨긴 border의 collapsed 오류선은 유지한다. 지원하지 않는 확장 자식은 출력 오류로 알린다.

추가 실제 앱 회귀는 여러 Area의 commit 사전 검증 원자성, 활성 자식을 마지막 자식으로 오인하지 않는 Calculate Expression, Area draft Undo/Redo, Enter→저장→열기의0실행/0Pending, 중첩 소유권을 유지하는 MCDX 왕복을 포함한다.

## 성능과 입력

Area의 각 자식이 같은 geometry cache를 덮고 실제 paint widget과 다른 cache를 쓰던 경로를 수정했다. 실제 child UI identity를 측정과 paint에 함께 사용하고 한 frame의 중복 측정을 제거했다. CPU counter 회귀에서1/2/8자식의 warm idle 조판은0회이고 font 변경 후 다시0회로 안정된다. 이것은 전체 앱 지연이나 실제 GPU 성능 개선률을 뜻하지 않는다.

Area가 있다는 이유만으로 빈 종이의 모든 포인터 입력에서 문서 전체 snapshot을 만드는 guard를 좁혔다. navigation/IME preedit는 source 준비·계산0회이고 Text는 확정 입력을 실행하지 않는 계약을 검사한다. resolved VARIABLE/UNIT 라벨도 가상 자식 ID를 통해 전달하며 캐시 hit에서는 Arc를 재사용한다. 구형 입력 adapter의 IME Disabled를 소비하면서 preedit를 지우지 않아 후속 Text를 버리던 호환 경로도 수정했다. 현대 egui의 empty Preedit 취소와 실제 OS IME 실기 검증을 구분한다.

아직 매 Text 이력에서 소유 Area XML을 복사하며 큰 Area commit의 반복 XML 검증, Area 존재 시 전체 worksheet DOM 준비 fallback이 남아 있다. 전반적 버벅임 해결을 주장하지 않는다.

## 검증 기록

증거 경로: `artifacts/rea/area-matrix-integration-20261019`, `area-matrix-windows-*-20261019.log`, `area-matrix-r137-*`. frozen source archive와 파일별 SHA manifest는 `freeze-area-matrix-r137.py`로 생성하고 peets에서 다시 검증한다. 전체 gate와 PDF/XML 검증의 최종 결과는 이 문서에 후속 기록한다. 준비 중인 기록을 통과로 간주하지 않는다.

**r137 최종 검증:** Windows workspace1147+backend2, peets workspace+backend1149, 실패0/ignored59. 두 OS의 fmt·workspace/all-target Clippy·backend Clippy·build와 reference38saved/38input 모두 통과. Windows release build도 통과했다. 행렬/Area native fixture 각 OS3개, 총6MCDX의 worksheet/header/footer/presentation24 XML part가 XSD 오류0이고, 원본1015/1016·실제 Area/행렬 앱 출력·Area helper를 포함한14PDF는 PyMuPDF/pypdf 독립 추출과 이미지0개를 확인했다. Area 실제 입력·저장 열기의0실행/0Pending 및 warm idle 조판0회 회귀도 포함한다. 실제 GUI/프린터 실기 검증과 구분한다.

소스 archive `mathcraft-area-matrix-20261019-r137.tar`:11,171,840bytes, SHA256 `4885493fb4b31e8d3ad149a221236a43e23541010d652c3d6ae90a31e237bc9b`; manifest SHA256 `73c96f29136f7cacc70abaf5c575f4f7d7408ce43b6e3c4f3695b55a0726d93b`,374파일/174product Rust. peets에서 archive와 모든 파일 hash를 재검증했다. 회수한634,880byte evidence tar SHA256 `06ce7fb3b03a84ceb0e45082255044535a66c44b4fe4407c7d6cdd7da6d9f54a`도 전송 양끝이 같다. 최종 `validation.json`에 gate log와 산출물별 SHA를 보관한다. archive 안의 이 문서는 검증 전 상태이며, 이 최종 기록은 이후 문서 갱신이다.

Windows frozen release `mathcraft-area-matrix-r137-20261019.exe`:22,807,552bytes/SHA256 `cc1ed414f1bf84596958f82d41ccb54caf6f12748b37c3124171a69c6d7b265b`. frozen CPU harness는23,382,016bytes/SHA256 `884b2f2f83d2983de486181613391fae3eb0f606ca85e6dbeedf687afaf91d8c`. 이전22,333,952byte r132 release보다473,600bytes 증가했다. 기능 확장에 따른 크기 증가를 숨기지 않으며 이번 묶음은 전체 실행 지연 개선이나 바이너리 축소를 주장하지 않는다.

## 남은 경계

신규 Area 작성/Ctrl+Shift+A, 자식 추가·삭제·다중 영역 cut/paste, 보호/암호, 전체 원본 영역 종류·queue·정밀 의존성·자식별 visible F5 경계는 미완료다. 부모 disable-calc 의미, 원본 border의 정확한 기하/색과 일반 collapsed border resource 조합은 미확정이다. unknown/protected/nested 입력을 추측하여 부분 실행하지 않는다. 기존553행 중 HD113–117은 지원 경로가 생긴 범위의 부분 구현으로 옮기고, 그룹화·보호 관련 행은 미구현으로 유지한다.
