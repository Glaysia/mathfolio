# XY 슬롯 키·계산 연결 검증

2026-10-19. [0179 원본 계약](0179-xy-slot-key-authoring.md)을 실제 키 입력, draft, 확정, 계산 결과 연결, 저장과 벡터 출력에 연결한 r184 고정 source 검증이다. Windows workspace1449+backend2, peets1451이 통과했다. 최종 정상 검사 실패0/ignored79, fmt·workspace/backend Clippy `-D warnings`·build·reference38saved/38input·Windows release를 확인했다. 이전 r180의 검증은 [0178](0178-xy-units-integration.md)에 보존한다.

양 OS90MCDX의450XML parts와 별도10XML, 합계460XML이 원본 XSD 오류0이다.56PDF는 이미지 객체0이며 fitz/pypdf 두 reader에서 텍스트를 추출했다. 전체 plots89회귀에는 실제 키16개가 포함되며, 일반 정상 검사와 별도 probe 수를 중복 합산하지 않는다. 원본 GUI·스크린샷·CLR/COM·Mathcad runtime 실행은 없다.

## 앱 수명과 명령 순서

[실제 컨트롤러 회귀](../../crates/mathcraft-app/src/worksheet_ui/plot_slot_key_integration.rs)는 X=[1,2,3], Y 목록=[2*x,5*x]에서 시작한다. 같은 OS 입력 묶음의 선택·3*x 치환·ShiftEnter·4*x 입력을 raw hook으로 나누어 처리한다. 확정 전 document, 이전 곡선, 문서 Undo, 계산 generation은 그대로다. Enter 이후 source는 [3*x,4*x,5*x]이며 한 문서 Undo다. 이전 첫/마지막 식 identity의 곡선만 Pending으로 유지하고 새 가운데 곡선에는 이웃 결과를 붙이지 않는다.

이어 붙은 F5·저장도 뒤쪽 텍스트보다 먼저 실행하지 않아야 한다. 실제 global editing shortcut과 page draw 순서를 사용하는 headless 회귀가 이 경계를 검사한다. 명시 계산 뒤 세 곡선은 각각 3*x,4*x,5*x이며 own/native 저장과 cold reopen에서 계산하지 않고 복원되어야 한다. 별도 Pending native 저장/복사에서는 세 resultRef 모두 Pending, 가운데 trace payload 없음, 이전 첫/마지막 곡선만 유지한다. PDF의 image object 및 텍스트 내용은 별도 검증 스크립트로 확인한다.

resize로 draft를 확정하는 경로도 source baseline 검사와 같은 identity remap을 사용한다. 슬롯 삽입 직전의 source와 크기를 한 region checkpoint로 보관하며, 새 pairing의 previous curve를 전부 버리거나 ordinal로 이식하지 않는다. 외부에서 source가 바뀐 draft는 resize candidate로 설치하지 않는다.

## 런타임 identity와 미확정 범위

혼합 이력은 구조 조작 이후 수정한 한 필드의 text/cursor 또는 history-free typed scaling snapshot을 보관한다. 매 키마다 전체 PlotSpec이나 native text Undo stack을 복사하지 않는다. 글자 입력 후 삭제하여 이전 값과 같아져도 최근 삭제가 먼저 Undo되어야 하며, 구조 Undo를 먼저 실행하지 않는다. 실제 구조 경계에서는 그 이전 native 편집 이력을 복원할 수 있어야 한다. IME preedit는 문서 이력으로 기록하지 않고 확정 Commit과 동반 Enter는 같은 입력 묶음으로 유지한다. 이 독립 앱 이력 정책을 원본의 모든 Undo grouping 시간 규칙과 동일하다고 주장하지 않는다.

슬롯 Undo/Redo/Cancel와 source commit 이후의 문서 Undo에서 equation ID floor를 되감지 않는다. 삭제된 plot owner도 문서 Undo/Redo에 남아 있으면 floor를 유지한다. floor 정리는 현재 문서와 두 history stack에 도달할 수 있는 owner의 합집합을 사용하며 idle/per-key마다 전체 history를 훑지 않는다. 문서 열기/새 문서는 별도 상태로 시작한다. 팔레트의 presenter 수명과 equation ID floor는 서로 다른 정책이며, 전체 영역 삭제 뒤 presenter 재생성의 팔레트까지 원본과 동일하다고 주장하지 않는다.

별도 정적 리뷰에서 generic document next_id가 Full Undo로 rollback되어 새 branch의 새 owner ID가 재사용될 수 있음을 확인했다. 이는 plot 안의 슬롯 ID floor만으로 해결되지 않는 UI incarnation 문제다. 기존 전체 문서 Undo 계약을 이번 단계에서 임의로 바꾸지 않았으며 후속 범위로 남긴다. [정적 리뷰](../../artifacts/rea/xy-slot-queue-static-review-20261019.md)에 재현과 근거를 기록한다.

일반 표현식 슬롯은 아직 plain String 편집이다. typed scaling의 구조 편집과 원본 EquationControl 전체를 동일시하지 않는다. 원본 첫 math의 label·scope·복합 표현식 보존, 일반 nary 편집, 전체 IME·navigator·legend/adaptive layout, 다른 도표 종류와 실제 GUI/GPU 반응성은 남은 작업이다. 함수 이름·REA 분석·회귀 수를 제품 완성률로 환산하지 않으며 사용자 평가15%에서40% 목표 작업은 계속된다.

기능 목록의 HD156은 이전 단계에서 typed scaling·물리량 변환·저장이 연결되었는데도 미구현 문구가 남아 있었다. [FEATURES](../FEATURES.md)와 JSON audit를 부분 구현으로 정정한다. 세부553행은 부분148/미구현405이며 범위가 늘거나 완성률이 한 단계 오른다는 뜻이 아니다. HD154/155는 현재 슬롯 작성 경계와 남은 typed expression 차이를 갱신하며 부분 구현을 유지한다.

## 고정 산출물과 실행 기록

archive `artifacts/rea/mathcraft-slot-keys-20261019-r184.tar`는12,820,480bytes, SHA `7cb58cd52f9ad311a50f784b181072aa7ca7cdbbafc7168bb3d29279651783af`다. manifest `artifacts/rea/slot-keys-integration-20261019-r184/source-manifest.json` SHA `a2ee5856936549480c3af926a461e3bd4bdf33bd53b260b91d17bfc59e0404db`,472files/230product Rust다. peets checkout은 `/home/peets/Projects/mathcraft-slot-keys-20261019-r184`이며 실행 전후 모든 manifest 파일을 검증했다.

회수 evidence `slot-keys-r184-peets-evidence.tar`는2,877,440bytes, SHA `eb96a8097822e49ba9b5b474537f6eb18446dec3f217be1de7ada84c28878000`로 양끝 같다. `artifacts/rea/slot-keys-integration-20261019-r184/validation.json`은 최종 정상 로그·산출물과 finalization 시점의 current product=archive를 확인한다. archive의 문서는 당시 준비 기록이며 이 최종 설명은 이후 갱신이다. 이후 제품 변경은 별도 revision으로 검증한다.

Linux 첫 보조 실행은 복사한 runner의 unit codec 환경 변수 이름 오타로 중단됐다. 두 번째 실행은 이전 probe의 exclusive 생성 출력과 충돌했다. 두 실패 로그·이전 출력은 보존했으며, 환경 변수를 고치고 새 출력으로 전체 보조 probe를 다시 통과했다. 고정 제품 source는 바뀌지 않았다. validation.json의 runner_retries와 scripts SHA에 이 과정을 분리 기록한다. 준비 ledger는 실행 전 script hash이며 수정 뒤 hash와 구분한다.

Windows release `mathcraft-slot-keys-r184-20261019.exe`는24,411,648bytes, SHA `ab50432e9789b39f5b0b061e694c98d1858cc9216ce4edc76382283b984d3b3a`다. headless harness26,717,696bytes, SHA `af608bf9de4f9b85c738dfdf89e9df998322a654a240221a1729000fd57e352c`다. release는r180보다114,688bytes 증가했다. 이를 속도 개선이나 전체40% 달성의 근거로 쓰지 않는다.

## 렌더 성능 실험의 기각

[epaint 메모리 예약 실험](../../artifacts/rea/epaint-reserve-candidate-20261019/README.md)은 제품에 적용하지 않았다. registry 원본과 별도 scratch 후보의 출력 bytes 동일성을 확인했지만, 32k marker tessellation 중앙값은42.6664→42.6442ms로 이득이 거의 없고 draw+tessellation은45.8775→46.2030ms로 악화했다. output capacity는80,995,936→117,509,728bytes로 약45% 증가했다. realloc 횟수 감소만으로 채택하지 않는다. 이 수치는 해당 격리 workload의 CPU 측정이며 현재 앱 전체의 성능·GPU·RSS를 뜻하지 않는다. marker CPU 병목과 실제 장치 반응성은 계속 남아 있다.
