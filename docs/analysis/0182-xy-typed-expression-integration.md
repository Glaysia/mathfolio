# XY 첫 수식의 편집·물리량·저장 통합

2026-10-19. [0181 계약](0181-xy-typed-expression-contract.md)을 첫 math 편집·계산·native 왕복·벡터 출력에 연결하는 typed-xy-r193 단계다. Windows workspace1484+backend2, peets1486검사가 통과했다. 최종 정상 실패0/ignored80, fmt·workspace/backend Clippy -D warnings·build·reference38saved/38input·Windows release가 통과했다. 양 OS98MCDX의490parts와 별도10XML, 합계500XSD 검사 오류0이며58PDF에서 image object0와 fitz/pypdf 텍스트 추출을 확인했다. 이전 완료 검증은 [r184](0180-xy-slot-key-integration.md)에 보존한다. GUI·스크린샷·원본 runtime 실행은 없다.

[실제 컨트롤러 회귀](../../crates/mathfolio-app/src/worksheet_ui/typed_plot_integration.rs)는 VARIABLE `x`와 문자 아래첨자 `dc𝛼`, physical X=[1,2,3]m, Y=x²에서 시작한다. X의 명시 배율은 in, Y는 m²다. native 저장 후 cold open에서 입력을 계산하지 않고 [2/0.0254,4]를 복원하며 native copy의 worksheet/result 바이트가 같다.

실제 End·ShiftLeft·Text·Tab으로 Y 지수와 단위 지수를2→3으로 수정한다. Enter 전 문서·계산 generation·저장된 곡선·문서 Undo는 그대로다. Enter 후 한 문서 Undo, 이전 곡선 Pending, source의 라벨·문자 아래첨자 보존을 검사한다. Undo/Redo와 Pending own/native 저장, own cold open의 실행0, 명시 재계산 후 [2/0.0254,8], native cold 결과 복원과 PDF 출력을 이어서 검사한다.

문서 history·삭제·open/new에서 첫 math 위젯을 퇴역시키며 이전 포커스와 preedit 임시 데이터를 제거한다. owner별 widget generation의 high-water는 현재/Undo/Redo에서 도달할 수 있는 owner 동안 유지하며 문서 history가 되감지 않는다. 일반 도표 draft cancel/외부 same-ID source 교체도 새 generation을 사용한다. scaling widget까지 같은 정책을 확장한 것은 아니다. backend IME event에는 session token이 없어 새 대상에 다시 포커스한 뒤 도착하는 과거 Commit의 완전한 귀속은 미확정이다. generic Full Undo의 새 branch owner ID 재사용도 별도 공백이다.

구조 Typed의 readonly 표시와 print는 Editor/레이아웃 cache를 사용한다. typed source는 legacy 문자열 parser로 환원하지 않는다. PreparedPlotQuery는 축별 resolve/parse를 한 번 수행하지만 샘플마다 제한된 IR 복제·치환·평가는 남아 있다. 이 구현만으로 전체 앱 속도나 GPU/RSS 개선을 주장하지 않는다. marker tessellation CPU 병목과 실제 장치 성능은 계속 남아 있다.

독립 정적 리뷰에서 writer가 plot 단독 깊이를 검사하지만 reader는 region wrapper를 포함해 검사하는 경계 차이를 발견했다. writer preflight도 최소 owning region을 포함한다. Group53개를 감싼 identifier는 실제 writer→reader 왕복하고54개는 저장 전에 거절하는 회귀를 추가했다. 원본보다 더 많은 깊이를 허용하는 기능을 만든 것이 아니라 앱 내부 저장·읽기 제한을 일치시킨 수정이다.

원본 보존과 직접 편집/계산을 구분하며 HD154/155/156은 부분 구현을 유지한다. 현재 세부553행은 부분148/미구현405다. 검사 수를 완성률로 환산하지 않고 사용자 평가15%에서40% 목표 작업을 계속한다.

## 고정 소스와 실행 산출물

archive `artifacts/rea/mathcraft-typed-xy-20261019-r193.tar`는12,963,840bytes, SHA `6dd64d9d2409a30d165c8c8979f7b5e1fe7bd64878c245f5bf7b0e39b60ee13d`다. manifest `artifacts/rea/typed-xy-integration-20261019-r193/source-manifest.json` SHA `ad2e2a2c92c7d6d864fe7d0a04fda3618dd93acf8842bd5cbb3513608a43df70`,482files/238product Rust다. peets `/home/peets/Projects/mathcraft-typed-xy-20261019-r193`에서 실행 전후 모든 manifest 파일을 대조했다.

`typed-xy-r193-peets-evidence.tar`는3,287,040bytes, SHA `c57b2dd7a1a1da31be6d9f1ae74950d8f02d91e95a589544f5eeeb4e2ad31e03`다.

`mathcraft-typed-xy-r193-20261019.exe`는24,613,376bytes, SHA `314fd05387207e70f619b1945f119d46f82e39646831dce94318558f14f8f168`다.

`typed-xy-r193-headless-20261019.exe`는27,138,048bytes, SHA `c85c31749e6663cc94a1ff10d5a1515768eb342d014708a18f699d1c988e254d`다.

`validation.json`은 archive 전체 bytes와 최종 정상 로그·출력, finalization 시점 current product=archive를 검사한다. 이후 제품 수정은 별도 revision으로 검증한다. archive의 문서는 준비 시점이며 이 최종 보고는 검증 뒤 갱신했다. r192는 첫 full fmt에서 module 선언 순서가 거절되어 보존했고, 선언 순서만 고친 r193을 새 archive로 검증했다. 제품 source archive와 실패 로그를 덮어쓰지 않았다.

Windows release는r184보다201,728bytes 증가했다. standalone release 크기와 headless 검사 성공을 실제 GPU/전체 속도나40% 완성의 근거로 사용하지 않는다.
