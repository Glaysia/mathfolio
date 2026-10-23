# 서식 있는 텍스트 선택 후 스크롤 입력 대기 수정

2026-10-23. 여러 휠 이벤트가 들어온 native 입력 묶음을 한 프레임에 처리하도록 수정했다. 서식 있는 Text가 선택된 상태에서 휠 이벤트가 한 개씩 대기하던 문제를 다룬다. 기존 단축키·계산 확정과 렌더러 설정은 유지한다.

## 원인과 변경

`PlotInputQueue`는 도표·일반 Text·서식 있는 Text의 입력을 함께 전달한다. 서식 있는 Text에서는 다음 문자 입력 전에 실제 편집 결과와 caret를 게시하기 위해 입력을 나눈다. 이 경로가 `MouseWheel`도 문자 수정처럼 한 개씩 나누었다. 따라서 한 native 묶음에 여러 휠 이벤트가 있으면 이후 프레임에 하나씩 남고, 새 휠 입력까지 뒤에 누적될 수 있었다.

변경은 RichText 모드에서 연속된 휠 구간을 그대로 한 번에 전달하는 것이다. 이벤트를 합성하거나 삭제하지 않고 delta·순서·modifier·phase를 보존한다. 문자·IME·키·클릭 앞에서는 계속 경계를 나누며, 기존 IME Commit/Enter 확인 태그의 판정은 바꾸지 않는다. 큐에 저장된 이벤트의 모드도 재분류하지 않는다.

## 확인한 동작

- 수정 전 큐 검사에서24개 휠 이벤트 중 첫 이벤트만 전달되고 나머지가 남는 실패를 재현했다.
- 수정 후24개씩 연속된 네 입력 묶음이 각 native 프레임에서 대기 없이 전달된다.
- IME Commit → 휠 구간 → Enter → 클릭 → Text의 게시 경계와 원래 확인 태그가 유지된다.
- 실제 App의 raw input hook·UI 본문·ScrollArea를 사용한 검사에서64개×4DIP 휠 입력을 한 번에 전달하고, 큐가 비었으며 전체256DIP 이동이 반영되는지 확인했다. 소스·저장 결과·계산 제출·선택·Undo 불변도 확인한다.

첫 App fixture는 exporter의 일반 Text를 rich 편집 가능 source로 가정해 실패했다. 기존 native rich Text fixture를 공유하도록 바로잡았으며, 이 실패를 큐 결함의 재현으로 세지 않는다. 수정 전 큐 실패와 수정 후 실제 App 검증을 구분한다.

## 렌더링 비교

같은 release 테스트 실행체에서 원본1015 계산서의 실제 App 조판·mesh를 창 없는 unread offscreen target에 그렸다. 디더링 on/off를 교대로 각3회, 총72frame 비교했다. 두 설정은 입력·mesh 수·vertex/index bytes·texture 전송량·보이는 수식 수와 소스 상태가 같았다.

선택된 어댑터는 DX12 Microsoft Basic Render Driver, DeviceType Cpu였다. warm-up 뒤 완료 대기 중앙값은 on197.162ms·off197.229ms로 개선 근거가 없어 제품 디더링 설정은 바꾸지 않았다. 이는 강제로 제출 완료를 기다리는 offscreen 측정이며 실제 창의 어댑터·화면 표시 시간·FPS 측정은 아니다.

## 검증 기록

큐·기존 tall matrix·실제 App 스크롤 관련4검사가 통과했다. 같은 frozen runtime source로 Windows2180·peets2181 정상 검사, fmt·workspace/backend Clippy·workspace 빌드·기존 참조38건을 통과했다. Windows release 빌드도 통과했으며 실행 파일은 `target/release/mathcraft.exe`에 반영했다. 전체 검사 수에는 선택 실행용 ignored 검사를 포함하지 않는다.

상세 로그와 frozen source·release 테스트 실행체·비교 자료는 로컬 `artifacts/rea/scroll-fix-20261023/`에 보존한다. GUI·스크린샷·원본 Mathcad 런타임은 사용하지 않았다.

이 변경은 확인된 입력 대기를 제거한다. 다른 선택 상태·문서·장치의 렌더링과 실제 표시 지연은 [성능 계획](../PERFORMANCE.md)에 따라 별도로 확인한다.
