# Mathcad 호환성 구현과 측정

**2026-10-23 Windows native 진단:** GUI 허용 후 실제 창의 어댑터가 DX12 CPU fallback임을 확인했다. 전체 App의 native 프레임 시작 간격 약219ms와 UI 약1ms를 분리했고, 빈 화면·latency2·AutoNoVsync를 비교했다. [결과와 제한](analysis/0209-windows-native-software-rendering.md)에 기록한다. 소프트웨어 렌더링 경로의 병목을 확인한 단계이며 해결 완료나 실제 표시 FPS로 판정하지 않는다.

**2026-10-23 스크롤 입력 수정:** [서식 있는 Text의 휠 입력 대기](analysis/0208-scroll-input-backlog.md)를 제거했다. Windows2180·peets2181 정상 검사, fmt·Clippy·workspace 빌드·참조38건과 Windows release 빌드를 통과했다. GUI·원본 런타임은 실행하지 않았다. 같은 release 테스트 실행체의 offscreen 디더링 비교에서는 개선 근거가 없어 제품 렌더러 설정을 유지했다. 이 결과를 실제 화면 FPS나 모든 스크롤 지연의 해결 판정으로 확대하지 않는다.

**UI 개편 전 기능 체크포인트:** [현재 보존 범위](CHECKPOINT-2026-10-20.md). 다음 작업은 [현대적인 UI와 기존 단축키 안내 개선](UI-MODERNIZATION.md)이다. 기존 r222 고정 검증은 [입력 행렬 편집·조판·인쇄](analysis/0207-typed-source-matrix-window.md)에 보존한다. 아래 이름 붙은 이전 검증은 당시 이력이다.

**XY 첫 수식 후속:** [0181 계약](analysis/0181-xy-typed-expression-contract.md)·[0182 통합](analysis/0182-xy-typed-expression-integration.md)에 typed source와 일반 expression-only 편집, label/contextual/문자 아래첨자 보존·물리량 pointwise 계산·Undo/Redo·Pending 저장·cold reopen·벡터 출력을 연결한다. typed-xy-r193은 Windows/peets 각각1486검사, 양 OS98MCDX/10별도XML·58PDF 검증을 통과했다. 원본 전체 navigator·IME·sampling/domain·legend와 marker CPU 공백은 남아 있으며 완성률로 환산하지 않는다.


현재 상태 갱신: 2026-10-20. 기능 판정은 [FEATURES.md](FEATURES.md), 최신 완료 고정 검증은 [r222](analysis/0207-typed-source-matrix-window.md)를 따른다. 이전 수치·실패 cohort는 당시 이력이다.

대상은 설치된 PTC Mathcad Express Prime 12.0.0.1이다. 오픈소스 Mathcad의 작성 경험을 목표로 한다. [작성 경험 기준](DESIGN.md)에 시나리오·원본 근거·독립 구현을 연결한다. 원본 DLL·Lisp·폰트·리소스를 배포물에 넣지 않는다. REA와 설치 파일은 개발 분석에만 쓰고 Rust 실행에 필요하지 않다.

## XY 슬롯 키·혼합 이력·배치 입력: r184

[0180 통합](analysis/0180-xy-slot-key-integration.md)의 고정 source를 Windows workspace1449+backend2, peets1451이 통과했다. 최종 정상 검사 실패0/ignored79, fmt·workspace/backend Clippy·build·reference38saved/38input·Windows release를 확인했다. 양 OS90MCDX/450XML parts+별도10XML XSD 오류0,56PDF 이미지0·두 reader 추출을 확인했다. 실제 슬롯 삽입/빈 삭제·방향별 포커스·Unicode suffix·문자/단위/구조 Undo·IME 확인 Enter·배치 F5/저장·pair identity Pending·cold 복원을 검증했다. Linux 보조 실행의 환경 변수 오타와 기존 출력 충돌은 실패 기록을 보존한 뒤 source 변경 없이 새 출력으로 재검사했다. 첫 math typed 편집·전체 navigator/IME·legend·marker CPU·GUI/GPU 반응성과40% 달성은 미완료다.

## XY 물리량·배율·보존 archive context: r180

[0178 통합](analysis/0178-xy-units-integration.md)의 같은 고정 source를 Windows workspace1407+backend2, peets1409가 통과했다. 실패0/ignored78, fmt·workspace/backend Clippy·build·reference38saved/38input·Windows release를 확인했다. 양 OS82MCDX/410 XML parts+별도10XML XSD 오류0,54PDF 이미지0·두 reader 추출을 확인했다. 물리량 X→pointwise Y·명시 배율/자동 residual units 구분·typed unit 입력/단일 확정/IME·v3 source association·native cold copy·own upstream/Area Pending과 A4 f64/f32 표현을 검증했다. 전체 단위 체계·슬롯 키·legend·marker CPU·GUI/GPU 반응성 및40% 달성과 구분한다.

## XY 독립 크기·여러 입력칸: r172

[0172 통합](analysis/0172-xy-geometry-slot-integration.md)의 같은 고정 source를 Windows workspace1355+backend2, peets1357이 통과했다. 실패0/ignored74, fmt·workspace/backend Clippy·build·reference38saved/38input·Windows release를 확인했다. 양 OS72MCDX/360XML parts XSD 오류0,50PDF 이미지0·두 reader 추출을 확인했다. graph/requested/actual f64·부재/정밀도·old model/cache/source adapter·실제 resize metadata Undo·모든 식 칸 입력·단일 확정·IME·원래 source ordinal을 검증했다. 단위/scaling·Shift+Enter·legend 실배치·큰 목록 overflow·원본 WPF 수렴·전체 GUI 성능·40% 달성은 완료 범위에 포함하지 않는다.

## 빈 XY·곡선별 결과·native 수명: r167

[0167 통합](analysis/0167-per-trace-plot-integration.md)의 같은 source를 Windows workspace1335+backend2, peets1337이 통과했다. 실패0/ignored71, fmt·workspace/backend Clippy·build·reference38saved/38input·Windows release를 확인했다. 양 OS62MCDX/310XML parts XSD 오류0,46PDF 이미지0·두 reader 추출을 확인했다. 빈 삽입/실제 키·부분 실패/정상 sibling·source/context 무효화·동일 개수 topology 변경·v1/v2 Pending·native old curve 제거와 cold reopen 계산0을 검증했다. native 오류 serialization·단위 scaling·다중 슬롯 입력·legend 실배치·WPF geometry·40% 달성은 완료 범위에 포함하지 않는다.

## XY 슬롯 모델·렌더 입력: r160

[0160 통합](analysis/0160-xy-slot-render-integration.md)의 고정 source를 Windows workspace1306+backend2, peets1308이 통과했다. 실패0/ignored68, fmt·workspace/backend Clippy·build·reference38saved/38input·Windows release가 통과했다. 양 OS46MCDX/230XML parts XSD 오류0,40PDF 이미지0·두 reader 추출을 확인했다. pure 슬롯 모델과 PH 저장, immutable 좌표/borrowed 렌더 입력, 실제16trace Manual 입력의 계산0을 검증했다. [0159](analysis/0159-xy-render-cost.md)의 Windows workload20개 비교는 개선·악화·추가 메모리를 함께 기록한다. 슬롯 keyboard UI·전체 GPU·40% 달성과 구분한다.

## 공유 XY 축·native 목록 보존: r156

[0156 통합](analysis/0156-shared-xy-axis-integration.md)의 같은 고정 source를 Windows workspace1293+backend2, peets1295가 통과했다. 실패0/ignored67, fmt·workspace/backend Clippy·build·reference38saved/38input·Windows release가 통과했다. 양 OS38MCDX/190XML parts XSD 오류0,38PDF 이미지0·두 reader 추출을 확인했다. 양방향 공유 식 실제 키/계산·Undo/Redo·own ID·native 목록/refs/cache와 이전 JSON/signature를 검증했다. Shift+Enter·전체 EquationControl·40% 달성으로 확대하지 않는다.

## Inline XY 확정·IME·cache 수명: r154

[0153 통합](analysis/0153-inline-xy-commit-integration.md)의 같은 고정 소스를 Windows workspace1280+backend2, peets1282가 통과했다. 실패0/ignored66, fmt·workspace/backend Clippy·build·reference38saved/38input와 Windows release가 통과했다. 양 OS 합산26MCDX/130XML parts XSD 오류0,34PDF 이미지0·두 reader 추출을 확인했다. 타이핑의 source/curve 불변·계산0, Manual Pending/Automatic Enter, actual Tab/IME/resize·Undo·저장과 중간 명령 종료의 stale Sync/reply 방지를 검증했다. 공유 축/전체 원본 일치나40% 달성을 뜻하지 않는다.

## Native XY source·결과·편집: r152

[0147 통합](analysis/0147-native-xy-integration.md)의 같은 고정 소스를 Windows workspace1251+backend2, peets1253이 통과했다. 실패0/ignored65, fmt·workspace/backend Clippy·build·reference38saved/38input와 Windows release가 통과했다. 양 OS 합산22MCDX/110XML parts XSD 오류0,28PDF 이미지0·두 reader 추출을 확인했다. 기본 N/N XY 식/서식·curve 캐시·cold open 실행0·edit commit/재계산·mixed Pending 보존·unsupported print block을 검증하며 공유 축/단위/전체 원본 일치나40% 달성을 뜻하지 않는다.

## 문서 흐름·native 결과: r145

[0145 통합](analysis/0145-document-flow-integration.md)의 같은 고정 소스를 Windows workspace1217+backend2, peets1219가 통과했다. 실패0/ignored63이며 fmt·workspace/backend Clippy·build·reference38saved/38input와 Windows release도 통과했다. 양 OS 합산16MCDX/80XML part XSD 오류0,22PDF 이미지0·두 reader 추출을 확인했다. 원본을 새로 실행하거나 GUI·스크린샷을 사용하지 않았다.

페이지 나누기는 blank Ctrl+Enter·단일 인접 제거·suffix 이동·한 Undo·native 좌표/height2·인쇄 편집선 제외를 연결한다. 현재 source-bound 숫자/복소수/비정방 행렬·명시 단위변환 결과가 native 재열기 계산0/Pending0으로 표시된다. 원본 raw 결과는 전체 source/context 동일성에서 유지하고 바뀐 문서는 이전 payload를 보존한 Pending으로 쓴다. 새 unitMonomial·range/symbolic/plot/structured diagnostics, 전체 공백/겹침 이동은 미완료다. 큰 Area의 bounded CPU 개선과 작은 사례 악화는 [0142](analysis/0142-area-lookup-cost.md)에 따로 기록한다. 40% 완료를 뜻하지 않는다.

## 새 Area 저작 — 2026-10-19

[0140 통합](analysis/0140-area-authoring-integration.md)의 r140 소스 고정본은 Windows workspace1166+backend2, peets1168, 실패0/ignored60이다. fmt·Clippy·build·reference38saved/38input과 새 Area의 실제 입력/확정/다음 수식/저장 왕복을 확인했다. 양 OS 합산10MCDX/50XML part XSD 오류0,16PDF 이미지0·두 reader 추출을 검증했다. 새로운 native 결과 writer·큰 Area 성능·페이지 나누기는 다음 작업이고 이 검증에 포함되지 않는다. 40% 완료를 뜻하지 않는다.

## 행렬·가져온 Area — 2026-10-19

[0137 통합](analysis/0137-area-matrix-integration.md)의 r137 소스 고정본은 Windows workspace1147+backend2, peets1149, 실패0/ignored59다. fmt·Clippy·build·reference38saved/38input, source-bound Area 및 실제 키 행렬 native 왕복을 검증했다. 양 OS 합산6MCDX/24XML part XSD 오류0,14PDF 이미지0·두 독립 reader 추출을 확인했다. 실제 GUI/프린터 실기 검증이나40% 완료를 뜻하지 않는다. 새 Area 저작은 r137 이후 다음 작업이며 이 검증에 포함되지 않는다.

## 편집 교체·formatted·Typeover — 2026-10-19

[0130 통합](analysis/0130-editing-repair-integration.md)에 r132를 기록한다. 일반 이항·Fraction·Power 기호 교체, formatted/suffix·명시/문맥 라벨 삭제와 결합, Fraction 전체/Power script의 확인된 Typeover와 실제 Enter/Undo/native를 연결했다. 두 단계 선택과 문서 변경을 구분하며 마지막 이름 삭제의 빈 식별자 버그를 고쳤다.

Windows/peets 각각 **workspace1056+backend2=1058개 통과·실패0·56ignored**다. 앱422·editor220, 동일198파일/제품Rust155/vendor5, native20/PDF18건/OS, MCDX40/XML160파트 XSD 오류0, PDF36 이미지0, 원본 reference38/38과 전체 원본 open oracle를 확인했다. r125→r132 첫/확인720case는 오류0이지만16/36·14/36event pair가 증가했다. 전체 속도 해결이나40% 완료를 주장하지 않는다. 새 행렬/Area 작업은 이 snapshot의 검증과 별도다.

## 구조 삭제·연산자 선택·서명 직렬화 — 2026-10-19

[0122 원본 계약](analysis/0122-structural-deletion-contract.md)·[0123 비용](analysis/0123-signature-emission-cost.md)·[0125 통합](analysis/0125-structural-deletion-integration.md)에 r125를 기록한다. 유효한 숫자·이름 결합/실패 시 Scale, 확인된 Group/Fraction/Power/단항 삭제와 실제 이항 기호 선택을 연결했다. 서명 JSON bytes·해시·저장 cache oracle를 유지한다.

Windows/peets 각각 **workspace1009 + backend2 = 1011개 통과·실패0개·51개 ignored**다. 앱406개·editor195개를 포함한다. 동일 archive의187파일·제품 Rust144개와 양 OS의 native18/PDF16건·원본 reference38/38을 확인했다. 최종 XSD/PDF·성능 교대 측정·source/release identity 및 미지원 범위는 [0125](analysis/0125-structural-deletion-integration.md)를 따른다. 전체 원본 삭제나20% 완료를 뜻하지 않는다.

## 일반 연산자 문자 입력·문서 열기 — 2026-10-19

[0118 원본](analysis/0118-normal-infix-entry-contract.md)·[0119 비용](analysis/0119-native-open-cost.md)·[0121 통합](analysis/0121-infix-entry-integration.md)에 r121을 기록한다. Normal 문자 분할/교체·Scope·음수 alternate·세 경로의 커서·Ctrl+- 종료와 Enter 확정을 연결했다. open 작업에서 package와 정상 fresh 표 visual을 재사용하며 기존 import oracle로 결과/서식/오류를 대조한다.

Windows/peets 각각 **workspace981 + backend2 = 983개 통과·실패0개·47개 ignored**다. native17/PDF15건/OS, MCDX34개/XML136파트 XSD 오류0개, PDF30개 이미지0개, 원본 reference saved/input 각각38/38이다. source181파일·원격 산출물77개·로그와 최종 소스를 대조했다. r117→r121 첫 비교360case와 행렬 cache 증가 확인용 별도360case를 모두 보존했다. 원본 열기 감소와 입력/행렬 증가·정확한 identity·미지원 범위는 [0121](analysis/0121-infix-entry-integration.md)을 따른다. GUI·원본 실행·전체20% 완료를 주장하지 않는다.

## 괄호·입력 컨테이너·직렬화 — 2026-10-19

[0114 원본](analysis/0114-parenthesis-adjacency-contract.md)·[0115 비용](analysis/0115-fused-serialization-cost.md)·[0117 통합](analysis/0117-parenthesis-integration.md)에 r117을 기록한다. `2(3)`을32로 만들던 오류를 고치고 Normal/Scope·APPLY/Scale·부분 UTF16·컨테이너·닫기·ArrowRight를 구분했다. 단일 buffer 직렬화는 원래 bytes/error/validation/cache/display 계약을 유지한다.

Windows/peets 각각 **workspace 954 + backend 2 = 956개 통과·실패 0개·45개 ignored**다. native16/PDF14건/OS, MCDX32개/XML128파트 XSD 오류0개, PDF28개 이미지0개, 원본 reference saved/input 각각38/38이다. source177파일·원격 산출물72개·로그와 r113→r1175회 교대360case를 대조했다. 정확한 source/release·성능/악화·미지원 범위는 [0117](analysis/0117-parenthesis-integration.md)을 따른다. GUI·원본 실행·전체20% 완료를 주장하지 않는다.

## 암시적 배율·함수 적용·확정 준비 — 2026-10-19

[0110 원본](analysis/0110-implicit-adjacency-contract.md)·[0111 비용/설계](analysis/0111-commit-preparation-design.md)·[0113 통합](analysis/0113-implicit-scale-integration.md)에 r113을 기록한다. 자동 인접 입력과 Ctrl+Shift+U의 Scale, 활성 ∘, 함수 RHS 적용과 Ctrl+J의 implicit 본문 보존을 연결했다. 명시적 *·Ctrl+- 이름 아래첨자·Enter/F5/Manual·표/단위 계약을 유지한다. eligible typed tree의 XML 왕복을 줄이며 exact bytes/display/full-DOM/cache/error oracle과 fallback을 대조했다.

Windows/peets 각각 **workspace 918 + backend 2 = 920개 통과·실패 0개·43개 ignored**다. native15/PDF13건/OS, MCDX30개/XML120파트 XSD 오류0개, PDF26개 이미지0개, 원본 reference saved/input 각각38/38이다. 모든 source 172파일·원격 산출물67개·로그 해시와 최종 r109→r1135회 교대360case를 대조했다. 정확한 source/release·성능/악화 수치와 미지원 범위는 [0113](analysis/0113-implicit-scale-integration.md)을 따른다. GUI·원본 실행·전체20% 완료를 주장하지 않는다.

## 선택 수식의 프로그램 삽입·프레임 중복 제거 — 2026-10-19

[0106 원본](analysis/0106-populated-program-insertion-contract.md)·[0107 비용](analysis/0107-frame-interaction-cost.md)·[0109 통합](analysis/0109-program-insertion-integration.md)에 r109를 기록한다. Scope는 선택 식을 While/For/If/Try의 primal BODY에 보존하며 기존 Program을 재사용한다. Return Left/Right·native 첫 caret와 Normal 명시 전체 값 문자 선택의 빈 연산자 교체를 구분했다. 한 Undo·Redo·예산 실패 원자성을 검사했다. 일반 partial/implicit parser·전체 nav/repair·직접 IF 원본 계산은 미확정이다.

Windows/peets 각각 **workspace 889 + backend 2 = 891개 통과·실패 0개·40개 ignored**, fmt·Clippy·빌드·원본 reference saved/input 각각38/38이다. native14건/OS·PDF12건/OS, MCDX28개·XML112파트 XSD 오류0개, PDF24개 이미지0개다. tar의 모든 regular source166파일·원격 산출물62개·로그를 hash 대조했다. 정확한 source/release identity·r105→r109 실제 입력5회 교대 수치와 악화 표본은 [0109](analysis/0109-program-insertion-integration.md)를 따른다.

미수정 Enter/IME의 불필요한 snapshot을 줄이고 정확한 현 frame intrinsic 높이를 재사용했다. pending 글자의 galley/row/mesh COW를 피하는 paint 경로는 최종 tessellated 색상·vertex/UV/index·clip을 대조하며 page+tessellation CPU도 별도 측정했다. 기존 단축키·Enter 확정·저장 결과·표/단위·native fallback을 유지한다. 원본/GPU 실행이나 전체20% 완료를 주장하지 않는다.

## Try/on error·삭제·표시 중복 파싱 제거 — 2026-10-19

[0103 원본 계약](analysis/0103-try-control-contract.md)·[0104 삭제](analysis/0104-try-deletion-contract.md)·[0105 통합](analysis/0105-try-control-integration.md)에 r105를 기록한다. 빈 Try와 Ctrl+J try, 두 BODY·header caret·초기 placeholder 삭제·header 본문 병합·단일 Undo·native 저장·오류 fallback을 연결했다. 숫자/단위·return/break/continue와 lazy handler를 검사했다. 원본 전체 오류 taxonomy·부분 local state rollback·일반 repair는 미확정이다. Ctrl+- 이름 아래첨자와 Enter 확정 계약을 유지한다.

Windows/peets 각각 **workspace 862 + backend 2 = 864개 통과·실패 0개·36개 ignored**, fmt·Clippy·빌드·원본 참조 saved/input 각각 38/38이다. native 13건/OS·PDF 11건/OS, MCDX 26개·XML 104파트 XSD 오류 0개, PDF 22개 이미지 0개다. source 142파일과 원격 산출물 57개·로그 해시를 대조했다. archive/release identity와 r101→r105 실제 입력 5회 교대·모든 악화 표본은 [0105](analysis/0105-try-control-integration.md)를 따른다.

[0102](analysis/0102-display-preparation-cost.md)는 ordinary 계산 준비의 기존 DOM을 표시 tree 생성에 재사용하고 원본 문서·label·오류·행렬 순서 oracle을 대조한 결과다. tree decode가 계산 준비 쪽으로 이동하므로 표시 단계 감소만을 전체 개선으로 부르지 않는다. 전체 Enter와 실행별 worksheet+display를 함께 측정했다. solve-block fallback, XML/label/signature 생성과 실제 GPU 비용은 남아 있다. GUI·스크린샷·원본 런타임은 사용하지 않았으며 20% 달성으로 판정하지 않는다.

## 다중 조건 분기·원본 행 배치·직렬화 비용 — 2026-10-19

[0099 원본 계약](analysis/0099-conditional-branch-contract.md)·[0100 비용](analysis/0100-xml-escape-cost.md)·[0101 통합 검증](analysis/0101-conditional-branch-integration.md)에 r101을 기록한다. flat IF 다중 분기·전체 alternate·BODY/native 정규화·원본 최소 if와 별도 else if/also if/else·Ctrl+J if 및 인접 분기 단어·한 Undo·header/test/body 세로 배치·operator hit를 연결했다. Ctrl+- 이름 아래첨자와 Manual/F5/Enter 계약을 유지한다. 일반 populated split/merge·TRY·전체 glyph 배치는 미완료다.

Windows/peets 각각 **workspace 822 + backend 2 = 824개 통과·실패 0개·33개 ignored**, fmt·all-target/backend Clippy·빌드를 통과했다. 원본 참조 saved/input 각각 38/38이다. native 12건/OS·PDF 10건/OS, MCDX 24개·XML 96파트 XSD 오류 0개, PDF 20개 이미지 0개다. source 137파일과 원격 산출물 52개·로그 해시를 대조했다. archive/release identity·r98→r101 실제 입력 5회 교대 수치와 한계는 [0101](analysis/0101-conditional-branch-integration.md)을 따른다. 전후 360case 및 양 OS 별도 36case가 준비/계산/generation 계약을 통과했다.

직렬화의 다섯 번 복사를 한 번으로 줄였으며 exact bytes/errors oracle을 유지했다. builder의 일부 warm 표본 악화와 실제 Enter 자료를 함께 기록한다. GUI·스크린샷·원본 런타임은 사용하지 않았다. 기존 부분 구현 137·미구현 416, 총 553행의 지원 깊이를 갱신했으며 전체 20% 달성을 판정하지 않는다.

## 프로그램 제어문·cold 준비·실제 Enter — 2026-10-19

[0096 원본 계약](analysis/0096-program-control-authoring-contract.md)·[0097 준비 최적화](analysis/0097-cold-worksheet-preparation.md)·[0098 통합 검증](analysis/0098-program-controls-integration.md)에 r98을 기록한다. Ctrl+J의 while/for/return/break/continue와 zero-arity leaf·원본 precedence·안쪽 삭제·한 Undo·native 저장을 연결했다. Ctrl+- 이름 아래첨자와 Manual/F5/Enter 계약의 회귀를 유지한다. if/else/try 계열의 pair repair와 populated 위치 일반 삽입은 미완료다.

Windows/peets 각각 **791 + backend 2 = 793개 통과·실패 0개·31개 ignored**, fmt·all-target 및 backend Clippy·빌드를 통과했다. 원본 참조 saved/input 각각 38/38이다. native 11건/OS·PDF 9건/OS, MCDX 22개·XML 88파트 XSD 오류 0개, PDF 18개 이미지 0개다. source 133파일과 원격 산출물 47개·로그의 해시를 대조했다. archive·release identity와 PDF 범위는 [0098](analysis/0098-program-controls-integration.md)에 있다.

4,000영역 cold builder는 28.376→21.794 ms였다. 이는 입력 전체 성능과 구분한다. 실제 Enter의 r94→r98 5회 교대 수치는 [0098](analysis/0098-program-controls-integration.md)을 따른다. 전후 360case 및 양 OS 별도 36case가 준비/계산/generation 계약을 통과했다. GUI·스크린샷·원본 런타임은 사용하지 않았다. 상세 553행은 부분 구현 137·미구현 416이며 전체 20% 달성을 판정하지 않는다.

## 라벨 순환·자동 복원·현재 입력 비용 — 2026-10-19

[0093 원본 계약](analysis/0093-label-cycle-contract.md)과 [0094 통합 검증](analysis/0094-label-cycle-integration.md)에 r94를 기록한다. Ctrl+Q의 정확한 순환·contextual 시작점·리본 자동 복원·단일 Undo·caret/suffix·None(`*`/`NONE`)의 native 보존과 제한된 문맥 추론을 연결했다. Typeover·override 호스트·전체 wildcard/CAS는 미완료다. Ctrl+- 이름 아래첨자·첫 Matrix Tab/Shift+Tab 및 Manual/F5/Enter 계약의 회귀를 유지했다.

Windows/peets 각각 **workspace 764개 + backend 2개 = 766개 통과·실패 0개·29개 ignored**다. fmt·all-target Clippy·별도 backend Clippy·workspace 빌드를 통과했다. 원본 참조는 saved/input 양쪽 38/38·차이 0이다. native 10건/OS·PDF 8건/OS를 검사했으며 MCDX **20개·XML 80파트**의 설치 XSD 오류는 0개다. PDF **16개 모두 이미지 0개**이며 두 reader에서 검색 텍스트와 원본 표 숫자·색상·기준점을 확인했다. 원본 전체 배치·글꼴·물리 프린터 동등성은 미검증이다.

최종 소스를 r89 before와 같은 PC에서 각각 5회 순서 교대했다. 캐시 있는 4,000영역 첫 Text는 **59.367→2.494ms**, Enter는 **13.737→60.982ms**, 각 실행 Text+Enter 합의 median은 **73.104→63.476ms**다. Arrow release는 **58.980→0.786ms**였다. 캐시 없는 큰 행렬의 기록한 open~Enter CPU 단계 합은 **97.749→98.815ms로 늘었다.** after 180case와 peets 별도 36case가 첫 이벤트 준비 0회·비편집 total 0회·수정 Enter 1회 계약을 통과했다. r93 결과와의 차이를 통제된 추가 개선으로 해석하지 않는다. 전체 GUI/GPU 성능이나 모든 확정 지연이 해결됐다는 뜻이 아니다.

source tar는 `mathcraft-label-cycle-20261019-r94.tar`, SHA-256 `dee4f7f880b43e1d76b6e9ee4da1fa18a397e695114331017c91b6ddfb10f284`다. 제품 Rust 106개·vendor 5개를 포함한 129파일을 archive·양 호스트와 대조했다. peets root는 `/home/peets/Projects/mathcraft-label-cycle-20261019-r94`다. Windows release는 **22,016,000 bytes**, SHA-256 `3c7b28e0b4394d2e32263efc4be4d5aefc61517ad156b23796af42a4a1f30123`다. ignored `label-cycle-integration-20261019/source-validation.json`과 `peets/verification.json`에 산출물 42개·로그·성능 source identity를 기록했다. GUI·스크린샷·원본 런타임은 사용하지 않았다. EA-016은 부분 구현이며 상세 553행은 부분 구현 136·미구현 417이다. 전체 20% 달성을 판정하지 않는다.

## 실제 열기 뒤 첫 입력·확정·포커스 — 2026-10-19

[0092](analysis/0092-committed-input-integration.md)에 r93의 source·측정·산출물과 제한을 연결한다. 동일 소스로 Windows/peets 각각 **workspace 738개 + backend 2개 = 740개 통과·실패 0개·28개 ignored**다. app 334개·compat 249개·document 70개·editor 72개·core 13개이며 fmt·all-target Clippy·별도 backend Clippy·workspace 빌드를 통과했다. frozen 원본 참조는 saved/input 양쪽 38/38·차이 0이다.

Ctrl+- literal 아래첨자·Manual/F5·draft 확정 계약을 유지하면서 키 해제/커서 이동의 전체 source 준비를 제거하고 첫 Matrix Tab/Shift+Tab 누락을 고쳤다. 첫 수정은 이전 입력의 raw snapshot만 보관하고, 실제 이전 값 계산이 필요한 명령에서 준비한다. [0090](analysis/0090-first-open-input-cost.md)의 동일 PC 5회 교대 측정에서 저장 cache가 있는 4,000영역 첫 Text는 **66.081→2.954ms**, Enter는 **15.196→69.195ms**, 각 실행 Text+Enter 합의 median은 **81.277→71.854ms**다. Arrow release는 **64.528→0.942ms**였다. Enter의 전체 문맥·표시 트리 비용은 남는다. 최종 Windows 180case와 peets 별도 36case가 첫 이벤트 준비 0회·비편집 total 0회·편집 Enter 1회를 통과했다. CPU 검사이며 실제 GPU frame rate나 전체 계산 완료 시간으로 확대하지 않는다.

source tar는 `mathcraft-input-baseline-20261019-r93.tar`, SHA-256 `1e4d903b380f86eb784e97a8c93b5b30927f644a6b75d8bc3e59a4d088946c93`다. 제품 Rust 103개·vendor Rust 5개를 포함한 manifest 126파일을 archive·양 호스트와 대조했다. peets root는 `/home/peets/Projects/mathcraft-input-baseline-20261019-r93`다. Windows release는 **22,008,832 bytes**, SHA-256 `3c9b5d684ef5b3f8aeee6284e0ba184e4f80c9fe165db703b34922abe97626e4`다.

양 OS native 9건·PDF 7건씩, MCDX **18개·XML 72파트**의 설치 XSD와 원본 표 계산·색상·저장 표시 검사를 통과했다. PDF **14개 모두 이미지 0개**이며 두 reader에서 검색 텍스트를 확인했다. ignored `input-baseline-integration-20261019/source-validation.json`과 `peets/verification.json` 등에 source·산출물 37개·표준 원격 로그 5개·별도 lifecycle 로그의 해시를 기록하고 수집 후 검증했다. GUI·스크린샷·원본 런타임은 사용하지 않았다. 전체 제품 20% 달성을 판정하지 않으며 기능 553행은 부분 구현 135·미구현 418이다.

## 프로그램 지역 함수·키 입력·서명 비용 — 2026-10-19

동일 r89 소스로 Windows와 peets 각각 **workspace 722개 + 입력 백엔드 2개 = 724개 통과·실패 0개·27개 ignored**를 확인했다. 앱 318개, compat 249개, document 70개, editor 72개, core 13개다. fmt·전체 타깃 Clippy(`-D warnings`)·workspace 빌드 및 별도 backend library/test Clippy를 통과했다. Windows는 release 빌드이며 main 앱을 띄우지 않았다. frozen 원본 참조 38개는 saved/input 양쪽 각각 38/38·차이 0이다.

peets 소스는 `/home/peets/Projects/mathcraft-local-functions-20261019-r89`, source tar SHA-256은 `ed5f005baafecc34b2595c9acded5edb87923ca7736384dd540014e663fb5ca0`다. 제품 Rust 100개·OSS backend Rust 5개와 Cargo/lock·검사 스크립트·vendor provenance/licenses 등 123개 manifest 파일을 archive 및 양 호스트 소스와 바이트 대조했다. Windows `target/release/mathcraft.exe`는 **21,992,448 bytes**, SHA-256 `0487f4a2d660f38e9a2adf15fc9d3f39336ef6977faa8ea276ee2cc8b0bb2185`다. manifest는 ignored `artifacts/rea/local-functions-integration-20261019-r89/source-validation.json`이다.

[0085](analysis/0085-local-function-authoring-contract.md)·[0086](analysis/0086-local-function-calculation-contract.md)의 원본 descriptor·writer·도움말의 실제 native 예제를 따라 named 지역 함수 선언을 연결했다. 프로그램 statement에 `f(x)`를 쓰고 원본 단축키 `{`를 누르면 Function LHS로 바꾸고 `←` 오른쪽에 입력한다. 같은 command는 이름·named Index/Row/Column의 LHS 방향·단일 Undo를 보존한다. 비Program·잘못된 target·표 expression-only 문맥·행렬 범위에서는 정의를 만들지 않는다. 문자 아래첨자 안의 `{`는 이름 문자다.

함수는 실제 인자·앞선 local·outer parameter를 지원하고 caller에 새 binding을 누출하지 않는다. formal 이름의 overlay를 소유 선언과 calculus lambda별로 분리해 sibling·외부 수식의 단위 이름을 바꾸지 않는다. 원본에서 재대입 후 capture 의미가 확인되지 않은 경우에는 실제 free binding 변경 뒤 함수 호출을 명시 미지원으로 처리한다. 같은 값·무관한 local·parameter shadow·실행하지 않은 branch는 오판하지 않으며 내부 array fill metadata와 real/imag signed zero의 변경도 감지한다. 일반 수치 비교의 `0=-0` 의미는 바꾸지 않았다. 함수값 alias·배열·고차 함수·완전한 재귀·원본 전체 capture/type 계약은 미완료다.

[0087](analysis/0087-local-function-integration.md)은 실제 Text 이벤트와 한 글자씩 `f(x){x^2`를 작성하고 새 줄에서 `f(3)`을 계산하는 경로, 함수 body의 초안·수식 경계 Enter·Undo/Redo를 검사한다. 이전 focus 요청이 남은 리본 명령이 다른 영역의 이력을 잡던 오류도 재현·수정했다. Define·Evaluate·InsertSymbol·Factorial의 문서/editor Undo/Redo가 실제 selected region에 일치한다. 기존 Ctrl+- literal 아래첨자·Ctrl+Shift+C Column 및 키 충돌 회귀를 유지했다.

[0088](analysis/0088-calculation-signature-cost.md)은 정확한 CanvasRegion snapshot의 JSON을 보존해 기존 v1 source bytes·SHA를 유지한다. 독립 공개 문서 API oracle과 JSON 전체 바이트·IEEE 값·순서·삭제·서식·settings·archive 변경·Plot fallback·예산 회복을 비교한다. 추가 retained payload는 4,000영역 약 4.07 MB이며 private cache의 32 MiB 한도는 RSS 한도가 아니다.

같은 PC의 headless test profile을 5회 교대 실행한 median에서 4,000영역 수정 준비는 **16.885→12.744ms**, 그중 서명은 **5.653→1.899ms**, 확정·worker 제출은 **23.871→21.798ms**였다. 행렬 100개의 확정·제출은 **12.067→8.548ms**였다. cold retain은 **64.634→69.770ms**로 악화됐고 실제 draft key frame은 0.983→1.061ms였다. 첫 준비·전체 지연이 해결됐거나 실제 GUI/GPU 체감·전체 계산 완료 시간이라고 확대하지 않는다. raw 표본과 각 측정 harness SHA는 0088에 연결한다.

양 OS 각각 native probe **9건**, PDF probe **7건**을 실행했다. 새 지역 함수 문서의 6개 영역·5개 질의는 9·7·8·8·6 V를 계산하고, 자체 저장을 열 때 계산 0회로 표시 5개를 복원하며 native 재수입 뒤 전부 Math·재계산 오류 0개다. 행·열 대입·이전 구조 연산자와 실제 1015/1016 표의 계산·색상·표시 cache 회귀도 유지했다. MCDX **18개·XML 72파트**가 설치 XSD를 통과했고 실제 문서의 XamlPackage 27개씩은 원본과 같다. PDF **14개 모두 이미지 0개**, 두 reader에서 검색 가능한 텍스트를 확인했다. 새 지역 함수 PDF는 각 2쪽·복원 표시 5개·벡터 drawing 984개다. 기존 표 숫자·색상·기준점 검사를 유지하며 원본 전체 배치 동등성은 미검증이다.

로그는 ignored `artifacts/rea/local-functions-r89-{windows-fmt,windows-clippy,windows-tests,backend-windows,backend-windows-clippy,windows-build,windows-reference,windows-probes,peets-check,peets-backend-clippy,peets-reference,peets-probes,peets-pdfs}-20261019.log`다. `local-functions-integration-20261019-r89/{schema-validation.json,pdf-verified/validation.json,peets/verification.json}`에 스키마·PDF·원격 산출물 37개와 로그 5개 해시를 기록했다. GUI·스크린샷·원본 런타임·실제 프린터는 사용하지 않았다. 기능 목록은 부분 구현 135·미구현 418로 유지하며 전체 제품 완성률 20% 달성을 주장하지 않는다.

## 행·열 좌변 대입과 전체 XML 재해석 비용 — 2026-10-19

동일 r84 소스로 Windows와 peets 각각 **workspace 691개 + 입력 백엔드 2개 = 693개 통과·실패 0개·26개 ignored**를 확인했다. 앱 307개, compat 233개, document 70개, editor 68개, core 13개다. fmt·전체 타깃 Clippy(`-D warnings`)·workspace 빌드와 별도 backend Clippy를 통과했다. Windows는 release 빌드이며 main 앱을 띄우지 않았다. frozen 원본 참조 38개는 양쪽 saved/input 각각 38/38·차이 0이다.

peets 소스는 `/home/peets/Projects/mathcraft-array-slices-20261019-r84`, source tar SHA-256은 `8ea524c6c29853d2e74d30b1af97bf2ad74e9e58b68be709995cfc7068071ceb`다. 제품 Rust 95개·OSS backend Rust 5개 및 Cargo/lock·검사 스크립트·vendor provenance/licenses 등 118개 manifest 파일을 archive 및 양 호스트 소스와 바이트 대조했다. Windows `target/release/mathcraft.exe`는 **21,905,408 bytes**, SHA-256 `796e348b7cd8c90c9f261127e6be1ee24b59ab9d9e1276f2e00ce2761a56d711`다. manifest는 ignored `artifacts/rea/array-slices-integration-20261019/source-validation.json`이다.

[0081](analysis/0081-array-slice-definition-contract.md)의 원본 descriptor와 [0082](analysis/0082-array-slice-definition-calculation.md)의 setter·Lisp 증거에 따라 named Row/Column 좌변을 일반·전역·프로그램 대입에 연결했다. scalar index와 ORIGIN, 짧은 RHS의 prefix 갱신·기존 셀 유지·필요 크기 확장, alias/capture의 불변성, physical fill metadata, 오류 binding과 취소를 검사한다. Row matrix block RHS는 저수준 setter의 직접 증거이며 상위 typing 전체 일치는 미확정이다. 원본 range lowering가 미확정인 slice range index는 명시적으로 거절하고 기존 일반 indexed range는 유지한다.

[0083](analysis/0083-array-slice-integration.md)은 초안·Enter·Undo/Redo·실패 전파를 연결한다. 잘못된 slice 초안이 기존 성공한 행렬 값을 계속 보여 주는 경로를 막았다. 리본 Define·Evaluate·InsertSymbol이 문서 전체를 이력에 복사하던 경로도 region 이력으로 바꿔 다른 live editor의 초안을 보존한다. 해당 복사 제거 자체의 시간은 별도로 측정하지 않았다. Ctrl+- literal 아래첨자, Ctrl+Shift+C Column 및 기존 키 충돌 회귀도 전체 검사에 포함된다.

[0084](analysis/0084-prepared-source-cost.md)는 builder가 직접 낸 UTF-8 범위를 활용해 warm 수정 시 전체 worksheet DOM 재해석을 제거한다. 기존 builder 문자열과 독립 전체 DOM oracle의 차등 검사를 유지하며 cold 초기화·Solveblock은 전체 DOM 경로를 사용한다. 같은 PC의 headless 단일 전후 표본에서 4,000영역 수정 준비는 **22.131→15.312ms**, 확정·worker 제출은 **26.046→21.915ms**, 16×16 행렬 100개의 제출은 **16.484→11.118ms**였다. cold retain은 **60.808→62.308ms**로 늘었다. 실제 GUI/GPU 프레임률·전체 계산 완료 시간·RSS 측정으로 확대하지 않는다.

양 OS 각각 native probe **8건**, PDF probe **6건**을 실행했다. 새 13영역 slice 문서는 6개 질의 기대값·자체 저장 재열기 계산 0회·native 재계산 오류 0개를 검사한다. 실제 1015/1016 표의 80 계산 행·수정 42 A·복원 표시 64개도 유지한다. MCDX **16개·XML 64파트**는 설치 XSD를 통과했고 실제 문서의 XamlPackage 27개씩은 원본과 동일하다. PDF **12개 모두 이미지 0개**이며 두 reader로 텍스트를 확인했다. 새 slice PDF는 각 2쪽·표시 6개·벡터 drawing 317개다. 기존 색상·표 숫자·기준점 검사를 유지하며 전체 원본 배치 동일성은 미검증이다.

로그는 ignored `artifacts/rea/array-slices-{windows-fmt,windows-clippy,windows-tests,backend-windows,backend-windows-clippy,windows-build,windows-reference,windows-probes,peets-check,peets-backend-clippy,peets-reference,peets-probes,peets-pdfs}-20261019.log`다. `array-slices-integration-20261019/{schema-validation.json,pdf-verified/validation.json,peets/verification.json}`에는 스키마·PDF·원격 산출물 32개 및 로그 5개 해시를 기록했다. GUI·스크린샷·원본 런타임·실제 프린터는 사용하지 않았다. 기능 목록은 553행 중 부분 구현 135·미구현 418로 유지한다. 전체 Mathcad 완성률 20%를 달성했다고 판단하지 않는다.

## 백분율·도·극형식과 확정 준비 비용 — 2026-10-19

동일 r80 소스로 Windows와 peets 각각 **workspace 664개 + 입력 백엔드 2개 = 666개 통과·실패 0개·25개 ignored**를 확인했다. 앱 302개, compat 214개, document 70개, editor 65개, core 13개다. fmt·전체 타깃 Clippy(`-D warnings`)·workspace 빌드와 별도 backend library/test Clippy를 통과했다. Windows는 release 빌드이며 앱을 띄우지 않았다. 기존 원본 참조 38개도 saved/input 양쪽 각각 38/38·차이 0이다.

peets 소스는 `/home/peets/Projects/mathcraft-scalar-notation-20261019-r80`, 양 호스트 source tar SHA-256은 `1cadc59978bfe930566e6acd2e7cd9076e9cbc75da8f285cec5d2d0f31454504`다. 제품 Rust 92개·OSS backend Rust 5개와 Cargo 설정/lock·검사 스크립트·vendor provenance/licenses 등 manifest 115개 파일을 archive 및 원격 소스와 바이트 대조했다. Windows `target/release/mathcraft.exe`는 **21,879,296 bytes**, SHA-256 `c5307c9d229f847497242134ac388af5f7970bec5377c98c3c48ebc158778fbb`다. source manifest는 ignored `artifacts/rea/scalar-notation-integration-20261019/source-validation.json`이다.

[0077](analysis/0077-scalar-notation-contract.md)·[0078](analysis/0078-scalar-notation-calculation.md)·[0080](analysis/0080-scalar-notation-integration.md)에 따라 Percent·Degree·Polar의 원본 descriptor·키·XML·조판·계산을 연결했다. 기존 “도·분·초 연산자” 항목은 실제 원본인 단항 Degree로 이름을 교정했다. `%`는 0.01배 계산이며 결과 Percent 서식과 별개다. `°`는 π/180배, Ctrl+Shift+P는 ∠ 입력이며 일반 Ctrl+P 인쇄에 가로채이지 않는다. Polar의 확인된 무차원 overload와 일반 곱/벡터화 차이를 보존하고 물리 단위는 완성된 phasor 뒤에 곱한다.

일반 텍스트 입력, 한 번의 Undo, literal suffix의 %/° 문자, 행렬 범위 typeover 거절, Degree와 Percent의 서로 다른 Power grouping, 왼쪽 Polar/오른쪽 Cross의 결합을 검사했다. 리본 명령은 초안으로 남고 Enter에서 확정한다. 새 문서 probe는 `50%=0.5`, `90°=π/2`, `2∠90°=2i`, `200·10%=20`, `sin(90°)=1`, `(2∠90°)·V=2i V`와 2×2 행렬 Percent를 비교한다. 자체 저장 재열기는 표시 7개·계산 실행 0회·Pending 없음, native 재계산 오류 0개다. 모든 수식을 Math 입력으로 가져온다.

[0079](analysis/0079-commit-preparation-cost.md)의 성능 변경은 변하지 않은 MathML fragment의 문맥 라벨과 표시 tree 재생성을 줄인다. 정확한 원본 XML과 실제 identifier lookup의 값/부재가 같은 경우만 재사용하며 global 선처리·기하 순서·실패 poison·표·bound local 문맥은 유지한다. 별도 차등 audit 3개는 기존 builder와 정확한 XML 및 계산/오류 결과를 비교하고 잘못된 입력·문맥 변경·예산 초과 후 복구를 검사한다. 기존 labeller의 의미 보존 검사이며 원본 Mathcad의 전체 binder 의미를 검증한 것은 아니다.

Windows headless test-profile 측정에서 4,000영역 숫자 하나 수정 후 준비는 **41.097→20.717ms**, 실제 확정·worker 제출은 **46.349→27.600ms**였다. 실제 1015의 확정·제출은 2.954→1.650ms였다. 첫 cold retain은 50.945→57.803ms로 늘었다. 동일 실행 파일에서 순서를 번갈아 11회 측정한 라벨 단계 median은 cold 19.315→22.515ms, 수정 후 19.218→4.080ms다. 4,000개 선언의 보존 payload는 1,338,450 bytes이며 cache의 32 MiB 한도는 RSS 한도가 아니다. 이전 측정은 10월 16일, 재검증은 19일의 단일 CPU 표본이고 실제 GUI/GPU 성능 보장으로 확대하지 않는다. worker 잔여 계산 시간도 위 제출 시간과 구분한다.

양 OS 각각 **native 파일 probe 7건**과 **PDF probe 5건**을 별도 실행했다. 실제 1015/1016 표는 각각 80 계산 행·오류 0개·수정 값 42 A·복원 표시 64개를 유지했다. MCDX **14개·XML 56파트**가 설치 XSD를 통과했고 실제 문서의 XamlPackage 27개씩은 원본과 바이트 동일하다. PDF 10개는 원본 문서 4개, 구조 연산자 2개, 전역/벡터 2개, 새 숫자 표기 2개다. 새 숫자 표기 PDF는 각 1쪽·표시 7개·벡터 drawing 237개이며 두 PDF reader로 텍스트를 추출했다. 모든 PDF는 이미지 0개다. 원본 문서의 색상·표 숫자·기준점 검사를 유지하며 전체 글꼴/배치 동일성은 미검증이다.

로그는 `artifacts/rea/scalar-notation-{windows-fmt,windows-clippy,windows-tests,backend-windows,backend-windows-clippy,windows-build,windows-reference,windows-probes,peets-check,peets-backend-clippy,peets-reference,peets-probes,peets-pdfs}-20261019.log`다. 성능 로그는 0079를 따르며, 설치본 identity 재검사는 `scalar-notation-original-identities-20261019.log`다. 스키마·PDF·원격 산출물 27개의 해시 검사는 `scalar-notation-integration-20261019/{schema-validation.json,pdf-verified/validation.json,peets/verification.json}`에 있다. GUI·스크린샷·원본 런타임·실제 프린터는 사용하지 않았다. 전체 원본 편집·상징·nested 값·다른 문서 기능은 미완료이며 전체 제품 완성률 20% 달성으로 환산하지 않는다.

## 전역 정의·벡터화·외적 — 2026-10-16

동일 r76 소스로 Windows와 peets 각각 **workspace 621개 + 입력 백엔드 2개 = 623개 통과·실패 0개·22개 ignored**를 확인했다. 앱 289개, compat 191개, document 70개, editor 58개, core 13개다. fmt·전체 타깃 Clippy(`-D warnings`)·workspace 빌드와 별도 backend library/test Clippy를 통과했다. Windows는 release 빌드이며 실행 파일을 띄우지 않았다. 원본 참조 38개는 양쪽 saved/input 각각 38/38·차이 0이다.

peets 소스는 `/home/peets/Projects/mathcraft-global-vector-20261016-r76`, 양 호스트 source tar SHA-256은 `d69e2ef8cf58946c1026a906cc0010131be72a95928c2bd663806fdcf108d720`다. 제품 Rust 85개·OSS backend Rust 5개 및 Cargo 설정/lock·검사 스크립트·vendor provenance/licenses를 archive와 바이트 대조했다. Windows `target/release/mathcraft.exe`는 **21,817,344 bytes**, SHA-256 `9c95563cebafe1851162a683f4db33f1abc8cd6e0a4bc38f218207b32e42b142`다. manifest는 ignored `artifacts/rea/global-vector-integration-20261016/source-validation.json`이다.

[0073](analysis/0073-global-vector-operator-contract.md)의 descriptor·writer·키·root 제약에 따라 전역 정의, 벡터화, 외적의 전용 AST와 native MathML을 작성한다. [0074](analysis/0074-global-vector-operator-input.md)는 원본 glyph·외적의 오른쪽 결합·화살표 최소 폭과 4 DIP 배치 및 실제 앱 키 경로를 검사한다. [0075](analysis/0075-global-vector-operator-calculation.md)는 같은 shape·scalar broadcast·열 우선 오류/취소·사용자 함수의 원소별 호출과 3×1 복소/단위 벡터의 비켤레 외적을 검사한다. 새 원본 런타임 차등 검사가 아닌 정적 계약과 독립 Rust 회귀다.

[0076](analysis/0076-global-vector-integration.md)은 화면 아래쪽 전역이 위쪽 query에 적용되는 계산 대상과 F5 prefix, Enter 확정, 잘못된 전역 초안의 실패 전파, 다른 독립 식 계산을 연결한다. 표 header/unit/body는 원본 AllowExpressionsOnly context를 사용하며 정의 명령·붙여넣기 거절 시 dirty·Undo·계산을 만들지 않는다. context는 Undo 대상에 넣지 않는다. 전체 원본 편집 전이와 symbolic 변환, nested 값, 일부 전역 좌변·벡터화 특수식은 미완료다.

호스트마다 실제 1015/1016 표 수정·인덱스 정의·일반 행렬·기존 구조 연산자·새 전역/벡터 문서의 **native 파일 probe 6건**을 별도 실행했다. 새 문서는 10영역·표시 6개이며 자체 저장 재열기 실행 0회·Pending 없음·native 재계산 오류 0개다. 일반 곱 32와 벡터화 곱 [4,10,18], 외적 [−3,6,−3], 함수 호출 [3,6,9], broadcast [4,5,6]을 비교한다. 실제 문서는 각각 80 계산 행·오류 0개·수정 값 42 A·복원 표시 64개를 유지했다.

MCDX **12개·XML 48파트**가 설치 XSD를 통과했고 실제 문서 XamlPackage 27개씩은 원본과 바이트 동일하다. 별도 PDF **8개**는 원본 1015/1016 4개, 기존 구조 연산자 2개, 새 전역/벡터 2개다. 원본 문서는 A3 2쪽·표 숫자 55개·표시 64개·색상·원본 기준점을 유지한다. 새 전역/벡터 PDF는 각 2쪽·표시 6개·벡터 drawing 305개이며 두 PDF reader로 텍스트를 추출했다. 모든 PDF는 이미지 0개다. 전체 수식 페이지 경계 처리·글꼴·픽셀 일치의 증거로 확대하지 않는다.

현재 Windows headless test-profile 입력 probe 2개도 별도 실행했다. 4,000영역의 key frame 1.028ms·Enter submit 49.035ms, 실제 1015의 key frame 0.460ms·Enter submit 2.890ms였다. 4,000영역 drag 60 delta는 합계 97.145ms, drop 44.976ms이며 delta 동안 계산 제출 0회·drop 1회다. 단일 CPU 표본으로 실제 GUI/GPU 성능 보장이나 전후 속도 배율을 주장하지 않는다. 큰 문서의 확정 시 전체 입력 준비 비용은 여전히 남는다.

로그는 `artifacts/rea/global-vector-{windows-tests,windows-clippy,windows-build,windows-reference,windows-probes,peets-check,peets-reference,peets-probes,peets-pdfs}-20261016.log`, backend는 `global-vector-backend-windows{,-clippy}-20261016.log`/`global-vector-peets-backend-clippy-20261016.log`, 성능은 `global-vector-{input-latency,drag-latency}-20261016.log`다. 스키마·PDF 검사는 `global-vector-integration-20261016/{schema-validation.json,pdf-verified/validation.json}`이다. GUI·스크린샷·원본 런타임·실제 프린터는 사용하지 않았다. ignored 전체 실행이나 전체 Mathcad 완성률 20% 달성으로 환산하지 않는다.

## 구조 연산자·입력 백엔드 — 2026-10-16

동일 r72 소스로 Windows와 peets 각각 **workspace 581개 + 입력 백엔드 2개 = 583개 통과·실패 0개·21개 ignored**를 확인했다. 제품 workspace는 앱 273개, compat 176개, document 70개, editor 49개, core 13개다. fmt·전체 타깃 Clippy(`-D warnings`)·workspace 빌드 및 별도 백엔드 library/test Clippy가 통과했다. 기존 원본 참조 38개는 양쪽 saved/input 각각 38/38, 차이 0이다.

peets 소스는 `/home/peets/Projects/mathcraft-structural-operators-20261016-r72`다. source tar SHA-256은 양 호스트 `43d9063585a32352e178f1b001514e040f4332398b2b468199a8dced30dab164`로 일치한다. 제품 Rust 80개·OSS backend Rust 5개와 Cargo 설정/lock·검사 스크립트·vendor provenance/licenses를 archive와 바이트 대조했다. Windows release `target/release/mathcraft.exe`는 **21,725,184 bytes**, SHA-256 `d714eda1e02c2506a5dd5837d594811b44a7583153a03387f6c4678ff3231406`이다. 실행 파일을 띄우지 않았다. manifest는 ignored `artifacts/rea/structural-operators-integration-20261016/source-validation.json`이다.

[0069](analysis/0069-structural-array-operators-contract.md)의 원본 descriptor·BAML·factory에 근거해 Factorial·Transpose·Conjugate·Norm·행/열 추출의 전용 AST와 native MathML을 연결했다. 절댓값/행렬식 명령은 기존 `Absolute/absval` 단선, Norm 명령은 `Determinant/determinant` 이중선이다. 원본에 없는 새 `<norm/>`를 생성하지 않으며 이전 잘못된 native alias는 파서에서 제거했다. named `norm(...)`은 별도 함수로 유지한다. 같은 precedence라도 원본이 금지한 row/column/transpose 중첩은 grouping으로 구분한다.

[0070](analysis/0070-structural-array-operators-input.md)은 실제 키와 후위·윗줄·행/열 방향 첨자, source hit/caret와 읽기 전용 조판을 검사한다. 새 입력/geometry 회귀 10개를 포함한다. 입력 backend가 Ctrl+Shift+C를 일반 Copy로 바꿔 원래 Key를 버리는 결함은 [0072](analysis/0072-structural-operators-integration.md)처럼 OSS egui-winit 0.36.2의 C 판정만 교정했다. Copy+modifier로 키를 추정하지 않으므로 Windows Ctrl+Shift+Insert·전용 Copy·메뉴 Copy는 복사를 유지한다. upstream 원본 해시·라이선스·변경 notice를 보존하며 다른 source 파일 4개는 동일하다. global clipboard/history→수식 경로도 별도 검사했다.

[0071](analysis/0071-structural-array-operators-calculation.md)은 ORIGIN과 단위·복소수·방향을 보존한 matrow/matcol, factorial 경계와 열벡터 mapping, 비켤레 transpose, conjugate, magnitude/norm/determinant의 타입 분기를 검사한다. 실제 matrix 입력은 immutable binding을 빌려 읽고 선택된 출력만 소유한다. 새 계산 회귀 6개와 bridge 회귀 4개, editor 회귀 6개가 포함된다. `op_det`/`op_det2`의 최상위 물리 단위 type-lowering은 미확정이며 기존 단위 동작의 회귀를 원본 전체 허용 범위 증명으로 삼지 않는다.

별도 파일 probe는 호스트마다 실제 1015/1016 표 수정, 인덱스 정의, 일반 행렬 범위, 새 연산자 7개 문서의 **5건**을 수행했다. 실제 문서는 각각 80 계산 행·오류 0개·수정 값 42 A·복원 표시 64개, 인덱스 정의는 7영역·표시 3개, 행렬 범위는 2×3·표시 1개를 유지한다. 연산자 문서는 factorial 120·transpose 3×2·conjugate 2−3i·정방 세로선 연산 −2·row 1×3·column 2×1을 확인하며 자체 재열기에서 **표시 7개·실행 0회·Pending 없음**이다. MCDX 재계산도 오류 0개이며 입력이 Opaque로 떨어지지 않는다. 저장된 symbolic 결과의 새 조판 구조도 계산 없이 복원한다.

새 MCDX **10개·XML 40파트**가 설치 XSD를 통과했고 실제 문서 XamlPackage 27개씩은 원본과 바이트 동일하다. 양 OS의 실제 원본 1015/1016 PDF 4개는 각각 A3 2쪽·표 숫자 55개·캐시 표시 64개·색상·원본 기준점 검사를 통과했다. 새 연산자 PDF 2개는 각 1쪽·표시 7개·벡터 drawing 144개이며 두 독립 PDF reader에서 숫자/복소수 텍스트를 확인했다. PDF 6개 모두 이미지 0개다. 이는 전체 글꼴·수식 간격·원본 외형 일치의 증거가 아니다.

로그는 `artifacts/rea/structural-operators-{windows-tests,windows-clippy,windows-build,peets-check,windows-reference,peets-reference,windows-probes,peets-probes,peets-pdfs}-20261016.log`, 백엔드는 `structural-operators-backend-windows{,-clippy}-20261016.log`/`structural-operators-peets-backend-clippy-20261016.log`에 있다. 스키마·PDF 결과는 `structural-operators-integration-20261016/{schema-validation.json,pdf-verified/validation.json}`이다. ignored 21개 전체를 자동 실행하지 않았고 필요한 파일/PDF probe를 별도로 실행했다. GUI·스크린샷·원본 런타임·실제 프린터는 사용하지 않았으며 전체 제품 20% 달성으로 환산하지 않는다.

## 일반 행렬 범위·배열 참조 비용 — 2026-10-16

동일 r68 소스에서 Windows와 peets 각각 **549개 통과·실패 0개·20개 ignored**, fmt·전체 타깃 Clippy(`-D warnings`)·workspace 빌드를 확인했다. 앱 257개, compat 166개, document 70개, editor 43개, core 13개다. Windows는 release 빌드이며 실행 파일을 띄우지 않았다. 기존 원본 참조 38개도 양쪽에서 saved/input 좌표 각각 38/38, 차이 0이다.

peets 소스는 `/home/peets/Projects/mathcraft-matrix-workflow-20261016-r68`, 양 호스트 source tar SHA-256은 `e494015b96a1039fda02d7c45f1719890c2e17237e6ffed7fbdd61056ac221cd`다. Rust 76개와 root/crate Cargo 설정·lock을 archive와 바이트 대조했다. `target/release/mathcraft.exe`는 **21,647,872 bytes**, SHA-256 `0803137a55a6f54e1318c97dd133eed7598821703ad00277d8ee81bf0498cdf8`다. manifest는 ignored `artifacts/rea/matrix-workflow-integration-20261016/source-validation.json`이다.

[0065](analysis/0065-matrix-selection-contract.md)의 fresh REA는 일반 행렬 Typeover의 실제 Copy/Cut/Delete/Clear 및 가상 메서드 상속을 추적한다. [0066](analysis/0066-matrix-selection-application.md)은 포인터 직사각형 선택, 중첩 행렬, 확대·이동 좌표, clipboard와 Enter 확정을 연결한다. Delete/Backspace의 행·열·내부 내용·전체 1×1 분기와 리본 행/열 삭제를 구분한다. Clear는 크기를 유지한다. 원본이 거절하는 범위 Paste·일반 수식 입력은 독립 구현에서도 변경하지 않으며 Shift+Arrow에 새 직사각형 단축키를 만들지 않았다. 새 core 회귀 9개와 formula 회귀 5개를 포함한다.

[0068](analysis/0068-matrix-workflow-integration.md)은 리본 Clear 연결, 행렬/단일 서식 명령의 단일 영역 이력, 다른 편집기 초안의 Undo/Redo 보존과 초안 중 계산 억제를 검사한다. 새 파일 probe는 3×3 가운데 행 복사·Clear·Undo·행 삭제 후 2×3 값 `[1,2,3,7,8,9]`를 계산하고 자체 저장/재열기 및 MCDX export/import/재계산을 확인한다. 자체 재열기는 표시 1개·실행 0회·Pending 없음, native 재계산 오류 0개다.

[0067](analysis/0067-array-reference-cost.md)은 셀 하나의 index 조회가 배열 전체를 복제하던 비용을 제거한다. private immutable binding을 Arc로 공유하고 조회는 값을 빌리며 공개 Value/JSON은 유지한다. 같은 Windows test-profile benchmark에서 16,384 indexed 조회는 **4,768.0→6.0ms**, 캡처 함수 호출은 **10,495.9→17.2ms**, range 출력은 **4,658.1→13.5ms**였다. alias·unit fill metadata·capture·checkpoint·취소·공개 owned 결과 격리 회귀를 포함한다. 이 수치는 전체 앱 지연 보장이나 원본의 메모리 모델 비교가 아니다.

별도 파일 검사는 호스트마다 실제 1015/1016 표 수정, 인덱스 정의, 새 일반 행렬의 4개 probe를 실행했다. 실제 문서는 각각 80행·오류 0개·수정 값 42 A·복원 표시 64개를 유지하며, 인덱스 정의는 7영역·표시 3개·자체 재열기 실행 0회를 유지한다. 새 MCDX 총 **8개·XML 32파트**가 설치 XSD를 통과했고 실제 문서의 XamlPackage 27개씩은 원본과 바이트 동일하다. PDF renderer는 이번에 바꾸지 않았으며 벡터 PDF의 기존 r64 검증은 아래 해당 기록을 유지한다.

로그는 `artifacts/rea/matrix-workflow-{windows-tests,windows-clippy,windows-build,peets-check,windows-reference,peets-reference,windows-probes,peets-probes}-20261016.log`다. 스키마 결과는 `matrix-workflow-integration-20261016/schema-validation.json`이다. 전체 ignored 20개를 자동 실행한 것은 아니며 이번에 필요한 파일 probe와 배열 benchmark를 별도로 실행했다. GUI·스크린샷·원본 런타임·실제 프린터를 사용하지 않았다. 전체 선택/편집·결과 viewport·ScriptableControl·OS clipboard·GPU/IME 및 다른 미구현 기능은 남아 있으며 전체 제품 20% 달성으로 환산하지 않는다.

## 재현 가능한 기준

`tests/reference/mathcad12.json`은 우리가 생성한 최소 워크시트 38개를 원본에서 새로 열고 계산 완료를 확인한 결과다. 입력/출력 MCDX SHA-256, 원본 버전, 생성한 MathML, 저장된 좌표와 resultRef, 표시 수치·오류 태그·경고를 포함한다. ZIP 원본과 원본 제품 리소스는 Git에 넣지 않는다.

```sh
cargo run -p mathcraft-cli --locked -- conformance tests/reference/mathcad12.json
```

`mathcraft-compat`는 문자열로 평탄화하지 않고 MathML 구조와 라벨을 직접 평가한다. 비교 항목은 표시 실수/허수/열벡터, 오류 family, 이전 오류 참조의 존재 여부, 재정의 경고다. 수치 허용 오차는 `max(1e-12, abs(expected)*1e-8)`이다. 원본이 저장한 좌표와 원본에 입력한 좌표+문서 격자 설정을 각각 실행한다. Pending·실패·비어 있는 결과를 정답으로 세지 않고, 차이가 있으면 CLI는 비정상 종료한다.

현재 38개 표본이 두 경로에서 일치한다. 이 수치는 전체 Mathcad 호환율이 아니다. 정확한 오류 인자/노드 handle/ID, 출력 차원 자체, 모든 숫자 branch와 정밀도, 캐시·취소·수동 계산 수명 주기는 이 수치에 포함되지 않는다. 동일 위치 두 영역의 일반적인 tie 순서도 아직 확정하지 않았다.

확인한 차이에서 구현을 고쳤다.

- `a:=2; f(x):=a*x; a:=3; f(4)`는 **8**이다. 함수가 정의 당시 환경을 가진다.
- `0^0`은 이 원본에서 **1**이다.
- 계산 순서는 XML 저장 순서와 분리되어 위치에 연결된다. 저장 시 region/result ID도 바뀔 수 있으므로 출력 worksheet의 resultRef로 결과를 연결한다.
- 문서를 열 때 저장된 종이/격자의 교차점에 배치한다. 20과20+1e-8이 같은 격자로 이동하는 사례를 통해 단순 Y 정렬의 오답을 발견했다. REA로 로딩→AddItemAtLocation→AlignmentGrid 경로를 확인해 수정했다. 정확히 절반이면 아래쪽 교차점으로 맞춘다.
- VARIABLE ORIGIN 재정의와 SYSTEM ORIGIN은 같지 않다. CONSTANT i는 실제 허수 literal이 아니며 원본에서 undefined다. 표본 이름이 imaginary-square/complex-abs여도 그 입력의 실제 결과는 undefined로 남긴다.
- 실패한 재정의는 이전 정상값을 조용히 되살리지 않는다. 미지원 RHS와 중복 global도 종속 계산을 오류로 막는다.

## 새 기반 계층

`mathcraft-document::native`는 압축 항목의 원본 바이트를 유지하며 영역 계층, namespace·라벨·아래첨자, 좌표·크기·align 속성, 캐시 상태와 오류를 조회한다. 알 수 없는 XML/binary/relationship도 보존한다.

```sh
cargo run -p mathcraft-cli --locked -- native-inspect INPUT.mcdx
cargo run -p mathcraft-cli --locked -- native-copy INPUT.mcdx NEW.mcdx
cargo run -p mathcraft-cli --locked -- native-run INPUT.mcdx
```

`native-copy`는 새 파일만 만들고 재오픈해 각 항목 이름·내용·압축 방식을 비교한다. 압축 컨테이너 전체 바이트 및 모든 ZIP extra field의 동일성은 보장하지 않는다. ZIP64·암호화·다중 볼륨·일부 압축 방식은 명시적으로 거부한다. 실제 detune의 33개 항목 보존을 확인했다. 편집 export는 아래 범위로 구현했으며 원본 앱 재열기는 검증하지 않았다. 원본 문서는 변경하지 않았다.

`native-run`은 캐시를 읽어 답으로 사용하지 않는 새 Rust 계산이다. 기본 수식·정의에 더해 행렬 연산, ranges, 프로그램, 수치 미분·적분·root, 일부 통계, scalar equality 풀이블록, 제한된 유리수·다항식 상징 계산을 연결했다. SI 단위 212개·물리 상수 15개와 SYSTEM 5개 기본값을 반영했다. 정확한 타입·알고리즘 범위는 [ENGINE.md](ENGINE.md)와 분석 0005–0013에 기록한다. 영역 계층 전체 scope, 문자열, 전체 built-in/unit catalog, affine 단위, 전체 풀이·CAS는 미완료다. 미지원 구조를 0이나 이전 값으로 대체하지 않는다.

원본의 `label-is-contextual=true`는 명시 라벨과 다르다. 저장 정보는 보존하고 `worksheet_xml`이 만드는 계산 사본에서 제한된 구조·이름 규칙으로 다시 분류한다. 직접 MathML 실행에서 해결되지 않은 문맥 라벨은 cached label을 강제하지 않고 미지원으로 보고한다. 전체 자유 식별자 해석 우선순위는 미확정이다. [0016](analysis/0016-contextual-labels.md) 참고.

`mathcraft-editor`는 안정 ID의 AST arena, 캐럿·부분식 선택, 분수·지수·괄호·이항 연산 삽입, scope 방향·확장 이력·진입 커서 복원, 괄호 나가기, undo/redo와 명령 단위 편집 그룹을 구현한다. 선택한 부분식의 좌우 방향에 따른 피연산자 위치를 반영했다. 근거는 `docs/spec/mathcad12-editor-contract.json`, 분석 0002·0014다. 부분 피연산자 범위와 전체 우선순위·키 전이는 미완료다.

앱 본체는 빈 종이 위에 수식·텍스트를 배치하는 워크시트다. 수식 입력→구조 편집→정의/평가→동일 영역의 결과 표시→위치 이동에 따른 재계산→저장/재열기가 연결된다. 정의 RHS의 `=`와 반복 평가 요청을 분리했고, 선택 치환·붙여넣기·텍스트 이벤트 Undo를 검사한다. `:` shortcut과 숫자 뒤 단위 입력 등은 제한된 정책이며 원본 전체 키 문맥을 재현하지 않는다. 실제 창을 띄우지 않는 egui context와 순수 상태 전이 테스트를 사용한다. 현재 기본 실행 파일은 `target/release/mathcraft.exe`다.

`mathcraft-document::canvas` v2는 수식의 PortableTree와 좌표·텍스트·opaque 영역을 저장한다. 내부 arena/caret/undo는 저장하지 않는다. MCDX를 가져오면 원본 ZIP 바이트도 보존한다. 편집 불가/계산 미지원 영역은 raw로 남기고, 계산 의미가 있는 미지원 math wrapper 속성을 제거하여 실행하지 않는다. 자체 `.mathcraft` v2 저장과 새 파일로 MCDX 편집 export를 지원한다. export는 원본 parts·관계를 보존하며 계산 캐시를 Pending으로 무효화한다. 원본 XSD에 대한 생성 구조 검사와 Rust 재열기를 검사했고, 원본 앱 재열기는 검증하지 않았다.

검증 결과는 아래 최근 검증 기록에 별도로 남긴다. 원본 GUI 재검증과 한글 IME 검증은 headless 검사에 포함되지 않는다. 새 수학 기능의 정적 계약 테스트와 기존 원본 실행 38개 표본을 구분한다.

## 다음 완료 조건

구조 편집의 부분 피연산자 범위·전체 원본 키 전이, IME·OS 클립보드, 선택·이동·계산 오류의 AST 위치 표시를 확장해야 한다. MCDX의 편집·캐시 무효화·관계 보존을 더 넓은 문서에서 검증하고 원본에서 다시 열어야 한다. 고급 계산과 전체 풀이/CAS·affine 단위도 미완료다. 제한된 기준 사례나 단위 테스트를 이유로 제품이 완성됐다고 판단하지 않는다.

## 검증 이력: 2026-10-15 재부팅 후

- Windows: workspace fmt·Clippy(`-D warnings`)·테스트·빌드 통과. 전체 190개 통과 후 직접 MathML 문맥 라벨 거부 회귀 테스트 1개를 추가하여 해당 8개 테스트도 통과했다. 이 시점의 테스트 합계는 191개다. 이전 문서의 합산 오류를 정정했다.
- peets(Kubuntu 26.04): 같은 소스 사본 `/home/peets/Projects/mathcraft-ux-20261015`에서 `scripts/check.sh` 전체 통과. 기존 소스 폴더를 덮어쓰지 않았으며 빌드 산출물 디렉터리만 재사용했다.
- 양쪽에서 기존 원본 38개 참조가 저장 좌표·입력 격자 좌표 두 경로 모두 일치했다. peets의 `engineering.xml` 20개 영역은 오류 0개이며, 값은 별도 engineering 통합 테스트로 검사한다.
- 설치 원본에 대한 unit facts 추출 `--check`는 212개 단위·15개 상수 사실과 일치했다. 피드백 분석은 설치 DLL 5개 및 CIL 메서드 123개의 원본 바이트 해시를 재검증했다.
- 원본 앱, GUI, 스크린샷, COM을 새로 실행하지 않았다. 이 검증은 전체 Mathcad 호환성이나 실제 IME 사용감을 입증하지 않는다.

## 작성 UI 개선 검증: 2026-10-15

Windows와 peets의 `/home/peets/Projects/mathcraft-ui-20261015` 소스 사본에서 fmt, workspace Clippy(`-D warnings`), 전체 테스트 **207개**, 빌드가 통과했다. 앱 테스트는 45개, editor 테스트는 19개이며 나머지는 기존 계산·문서 회귀 검사다. Windows 실행 파일 `target/debug/mathcraft.exe`도 갱신했다.

새 검사는 종이 전용 배율과 50%·100%·200% 클릭 좌표, 문자 드래그·UTF-16 선택·치환·삭제, 기하 기반 분수·행렬 수직 이동, 수식·텍스트 Undo 후 커서 위치에서 연속 입력, Esc→영역 이동→Enter, 도구막대가 있는 상태의 Tab 이동, 메뉴 열기→Esc→입력 복귀, IME 임시·확정·취소 이벤트를 포함한다. 분수 도구가 기존 선택 피연산자를 버리지 않는 동작도 검사했다.

UI 구조·타이포그래피·기하 이동은 독립적인 제품 정책이며 원본 전체 외형/키 전이와 같다고 주장하지 않는다. 실제 창·화면 캡처·원본 실행은 사용하지 않았다. 실제 Windows/Linux IME 후보창, OS 클립보드, 고해상도 화면에서의 최종 사용감은 실기 검증이 남아 있다. 구현 흐름은 [작성 경험 기준](DESIGN.md)과 [사용법](PROTOTYPE.md)에 기록했다.

## 기본 공학 계산서 단계 검증: 2026-10-15

Windows와 peets의 /home/peets/Projects/mathcraft-m20-20261015에서 workspace fmt, Clippy(경고 금지), 전체 244개 테스트, 빌드를 통과했다. 앱 71개, editor 25개, document 28개 테스트를 포함하며 기존 원본 38개 참조의 두 좌표 경로도 유지된다. 실행 파일은 Windows target/debug/mathcraft.exe, Linux /home/peets/Projects/mathcraft/target/debug/mathcraft다.

새 검증은 함수·범위·도표 작성과 정의 수정/Undo에 따른 재계산, 축 편집 후 포커스, 행렬 행·열 삽입/삭제와 Shift 단축키, 서식의 저장·복사·Undo, 표시 반올림과 원본 계산값 분리, source hit ID, 확대 글꼴의 IME 좌표, 긴 행렬의 스크롤 범위, 숫자 일반 형식·0 표시 경계를 다룬다. 예제의 F=19.6133 N, det(A)=5, f(2)=8도 고정 검사한다.

수식 서식의 MCDX 출력은 설치본 worksheet50.xsd를 사용해 구조 검사를 통과했다. 누락된 XML namespace schemaLocation에만 오프라인 표준 XML 속성 선언을 연결했고 제품 스키마 선언은 유지했다. 원본 앱으로 재열거나 화면을 대조한 결과가 아니다. 이 당시 도표 MCDX export는 거부했다. 현재 기본 N/N native XY source·curve 저장/재열기는0147에 연결했으며 전체 source 규약은 부분 구현이다.

이 단계에서 GUI·스크린샷·원본 실행은 사용하지 않았다. 전체 제품의 20%라는 수치는 계측한 호환율이 아니라 기본 계산서 작성 흐름을 기준으로 한 개발 규모의 추정이다.

## 현재 범위 정리: 2026-10-16

프로그램/미적분/상징의 기본 구조 입력, 다중 trace, 제한된 Find/Minerr 편집, 머리글/바닥글 필드, lsolve·실수 대칭 고유계·유한점 다항식 극한을 추가했다. opaque 영역이 모든 결과를 막던 경로와 상징 결과 누락을 수정했고 Page/Draft를 연결했다. 원본 근거 없는 결과 확장 창은 제거했다. 기능별 남은 차이는 통합 목록에 기록한다.

최근 Windows 앱의 release 회귀 검사 90개와 앱 Clippy 경고 금지 검사가 통과했다. release 기본 실행과 idle 조판 개선의 측정 경계는 [0030](analysis/0030-rendering-performance.md)에 있다. 이 기록을 최신 workspace 전체 또는 최신 peets 검사 통과로 확대하지 않는다. 2026-10-15의 peets 244개 검증 이후 추가된 코드 전체의 Linux 재검증은 남아 있다. 이번 목록 정리는 문서·코드·기존 검사 대조이며 새 GUI/원본 실행 검증이 아니다.

## 기능 조사 확대와 판정 정정 — 2026-10-16

세부 동작·명령·설정 553행과 근거 파일 215개, 원본 계산 바인딩 21모듈·874행을 대조했다. 이번에 EfiAffine 바인딩 6개를 목록에 추가했고 release CLI `features --all`로 갱신 JSON의 로딩을 확인했다. 제품 계산 기능을 새로 구현한 검증이 아니다. [조사 기록](analysis/0031-feature-inventory-audit.md) 참고.

이 조사 시점에는 원본의 문서 열기·저장 결과 수명 주기와 Calculate Now의 가시 범위/활성 영역 계산이 미구현이었다. 이후 구현과 검증은 아래 기록 및 [0032](analysis/0032-calculation-commands-and-saved-results.md)를 따른다. 원본의 큰 기호 결과 폭 조절은 여전히 미구현이다. 기존 원본 38개 표본이 이 UI/계산 수명 주기를 입증하지 않는다.

## 저장 결과와 계산 명령 구현 검증 — 2026-10-16

같은 Rust 소스에서 Windows release 및 peets Linux의 workspace 테스트 **각 321개**를 통과했다. 앱 114개, 계산 호환 계층 127개, document 38개, editor 29개, core 13개다. 두 OS에서 fmt와 전체 타깃 Clippy 경고 금지 검사, workspace 빌드도 통과했다. Windows 실행 파일은 `target/release/mathcraft.exe`다. peets는 `/home/peets/Projects/mathcraft-calc-20261016-FI29GT`의 격리된 소스와 기존 target 캐시를 사용했다. 과거 244개 검증을 최신 결과로 대신한 것이 아니다.

회귀 검사는 열기/idle의 계산 실행 횟수 0, 저장 결과 signature 불일치·손상·Pending 보존, 실수/복소수/행렬/단위/기호 결과의 순수 복원, F5/Ctrl+F5 이벤트와 실제 가시 경계, 보이는 활성 영역 강제 계산, 변경 구간/전역 선행 조건, 깨진 마지막 영역 뒤의 정상 결과, 오류의 국소 전파를 포함한다. 500개 도표 표본의 문맥 재사용과 첫 수식보다 위에 있는 도표의 전역 참조도 검사했다. 최초 Linux 검사에서 발견한 headless 테스트의 `TexturesDelta` 미정리도 고쳐 최신 전체 검사를 다시 통과했다.

로컬 검증 로그는 ignored `artifacts/rea/calculation-workspace-windows-tests.log`, `calculation-windows-clippy.log`, `calculation-windows-build.log`, `calculation-peets-check.log`에 보존한다. 원본 REA 근거와 구현 경계는 [0032](analysis/0032-calculation-commands-and-saved-results.md)를 따른다. GUI·스크린샷·원본 런타임은 사용하지 않았다. 전체 native cache·정밀 의존성·비활성 영역·취소는 이 검증으로 완성 판정하지 않는다.

## 인쇄·성능·주요 입력/계산 누락 보완 — 2026-10-16

같은 소스로 Windows release와 peets에서 workspace **각 358개 통과, 3개 ignored**를 확인했다. 앱 141개, compat 128개, document 43개, editor 33개, core 13개다. ignored는 명시 실행하는 성능 benchmark 두 개와 로컬 native 문서 probe 한 개다. 두 OS의 fmt·전체 타깃 Clippy 경고 금지·workspace 빌드도 확인했다. peets 소스는 `/home/peets/Projects/mathcraft-print-20261016-r38`, build cache는 기존 `/home/peets/Projects/mathcraft/target`이다. 실제 GUI·스크린샷·원본 런타임·프린터 출력은 사용하지 않았다.

인쇄·용지/여백·날짜·프로그램 Enter·행렬 Tab·영역 비활성화 및 이전 문서의 비활성 상태 migration을 포함한다. 구조·glyph hit·표시 캐시 변경, 먼 페이지 jump와 수평 culling, 입력/Undo·계산명령 회귀도 통과했다. 이 구현 범위와 한계는 [0035 인쇄](analysis/0035-printing-implementation.md), [0036 입력](analysis/0036-program-lines-and-matrix-tab.md), [0037 비활성화](analysis/0037-disabled-regions.md), [0038 날짜](analysis/0038-saved-date-fields.md)를 따른다.

공학 예제의 수식·단위·행렬·도표를 headless PDF로 내보내고 독립 PyMuPDF 및 pypdf 파서로 검사했다. A4 595.2756×841.8897pt, 150dpi RGB 1241×1754px의 단일 페이지가 두 파서에서 복구 없이 읽혔다. 용지·방향·Custom 여백을 포함한 6개 MCDX의 worksheet/header/footer/presentation 총 24개 XML part도 설치본 XSD로 검증했다. 원본 앱 재열기나 실물 인쇄 품질의 검증은 아니다.

기존 로컬 `detune.mcdx`(87영역)는 원본을 실행하지 않고 열기 probe를 수행했다. 저장 표시 50개 복원, 표시 대상 Pending 없음, 계산 실행 0을 확인했다. 이 값은 해당 파일의 가져오기 결과이며 모든 MCDX의 완전한 지원을 뜻하지 않는다.

단일 Windows release headless CPU 샘플에서 4,000영역/12×12 저장 행렬의 120 idle frames는 215.696→136.718ms, pointer hold는 434.245→131.548ms였다. 1,000개 8×8 행렬의 동일 소스 update는 28.378→0.493ms, 전체 계산은 11.554→7.493ms였다. GPU·실제 사용자 입력 지연 측정으로 확대하지 않는다. 방법과 남은 비용은 [0034](analysis/0034-rendering-and-calculation-cost.md)에 있다.

최종 ignored 로그는 `artifacts/rea/printing-workspace-windows-tests.log`, `printing-windows-clippy.log`, `printing-windows-build.log`, `printing-peets-check.log`, `printing-native-result-probe.log`다. 독립 출력 검증은 `artifacts/rea/printing-validation/`에 보존한다. 첫 Linux Clippy에서 발견한 신규 테스트의 Default 후 field 재대입은 initializer로 고쳐 전체 검사를 다시 통과했다.

## 백그라운드 계산·중지/재개 및 경합 검증 — 2026-10-16

같은 최신 Rust 소스로 Windows release와 peets Linux에서 workspace **각 377개 통과, 3개 ignored**를 확인했다. 앱 154개, compat 134개, document 43개, editor 33개, core 13개다. 두 OS의 fmt·전체 타깃 Clippy(`-D warnings`)·workspace 빌드가 통과했다. peets 소스는 `/home/peets/Projects/mathcraft-background-20261016-r41`, target cache는 기존 `/home/peets/Projects/mathcraft/target`이다. Windows 최신 실행 파일은 `target/release/mathcraft.exe`(20,544,512 bytes)다.

이번 검사는 영속 계산 worker, 최신 소스 coalescing과 오래된 회신 거부, 새 문서 epoch, 수동 준비의 실행 0, 활성 정의와 후속 도표의 동일 범위 계산을 포함한다. 프로그램·수치·행렬·solve 내부 취소, 중단행의 이전 값/바인딩 rollback, 완료 prefix와 전역 정의 재사용, 저장 함수에 이전 취소 token이 남지 않는 경우도 검사했다.

별도 검토에서 찾은 두 경합을 결정적 callback gate로 재현해 수정했다. Auto OFF는 아직 시작하지 않은 자동 요청만 보류하고 명시 F5/Ctrl+F5 요청은 유지한다. 수동 계산 중 텍스트 삭제·복제·이동은 동일한 계산 작업을 prepare-only 요청으로 교체하지 않는다. Stop→MANUAL→Auto 재개에서도 이전 표시값을 유지한다.

원본 계약과 구현 경계는 [0039](analysis/0039-calculation-queue-and-cancellation.md), [0040](analysis/0040-calculation-cancellation-engine.md), [0041](analysis/0041-background-calculation-ui.md)를 따른다. 원본 위치별 의존성 queue 전체·영역별 즉시 결과 게시·외부 IO의 강제 중단은 구현하지 않았다. MathML 구성/Undo snapshot은 아직 UI 스레드 비용이다. 위 성능 수치는 취소 지점 추가 이전의 단일 샘플이며, 이번 변경 후 실제 GPU/사용자 입력 지연 측정으로 확대하지 않는다.

ignored 로그는 `artifacts/rea/background-workspace-windows-tests.log`, `background-windows-clippy.log`, `background-windows-build.log`, `background-peets-check.log`, `background-ui-race-tests.log`, `calculation-worker-windows-tests.log`다. GUI·스크린샷·원본 런타임·실제 프린터는 사용하지 않았다.

## 입력 확정·제공 PDF 비교·결과 서식 통합 — 2026-10-16

서식 통합 시점의 동일 Rust 소스로 Windows release와 peets의 workspace **각 405개 통과, 3개 ignored**, fmt·전체 타깃 Clippy(`-D warnings`)·workspace 빌드를 확인했다. 앱 176개, compat 134개, document 49개, editor 33개, core 13개다. peets 소스는 `/home/peets/Projects/mathcraft-background-20261016-r41`, target cache는 기존 디렉터리다. 당시 `target/release/mathcraft.exe`는 20,759,040 bytes였다. 이후 표·벡터 출력 변경의 검증은 다음 기록으로 구분한다.

입력 확정 검사는 일반 타이핑의 worker 작업 제출 0·Enter 후 한 번 제출, 이전 종속 결과 유지, IME/Undo·영역 이탈·수동 모드, Auto ON의 초안 제외, 가시 대상 없는 F5의 초안 보존·화면 밖 F5, 명시 `=`의 Auto OFF 대기 보존, 문서 저장의 Pending 상태를 포함한다. 원본 계약과 구현은 [0042](analysis/0042-edit-commit-contract.md)/[0044](analysis/0044-edit-commit-implementation.md)를 따른다.

사용자가 제공한 PDF 두 개의 전체 4페이지를 앱 GUI 없이 PDF 렌더로 비교했다. 같은 A3 2페이지이며 표·색상/강조 누락, Percent와 극형식 유실, 빈 근호 입력칸과 래스터 PDF의 차이를 확인했다. 독립 앱의 출력 경로를 고쳤으며 제공된 PDF는 변경하지 않았다. [비교 기록](analysis/0043-pdf-output-comparison.md)에 파일 SHA와 수치 표시 사례·비교 한계를 남겼다.

서식 검사는 Percent·후행 0·i/j·극형식·임계값·저장된 E 표기, native/default/local 상속, 계산값 불변, 원래 정밀도를 보존한 cold reopen→서식 재변경, MCDX export→import→명시 재계산, malformed numeric source 거부와 같은 인쇄 표현을 포함한다. 사전에 평가하지 않으면 실패하는 undefined 입력의 native cache도 서식 변경 중 실행되지 않았다. [원본 계약](analysis/0045-result-format-contract.md), [문서 모델](analysis/0046-result-format-persistence.md), [앱 통합](analysis/0047-result-formatting-integration.md)을 구분한다.

최종 소스에서 문서 49개 테스트를 다시 실행해 MCDX 7개를 재생성한 뒤 worksheet/header/footer/presentation **28개 XML part**를 설치본 XSD로 모두 검증했다. 보고서에는 각 출력 SHA와 최종 document 소스 3개의 SHA를 보존했다. 최종 `export.rs` SHA는 `994bd4fafe818b5aebaecab12f5447d28649873eb6d3a0576237cf97d89438a1`이며 peets 소스와도 일치했다. 이 검사가 원본 앱 재열기나 실제 프린터 출력을 입증하지 않는다.

별도 명시 실행 native probe는 로컬 `detune.mcdx`/`0818.mcdx`/`1kW_LCCL-S 설계.mcdx`에서 각각 50/45/39개 표시 복원, 표시 대상 Pending 없음, 계산 실행 0을 확인했다. ignored 검사는 위 405개 합계와 별도다. wrapper의 로그 리다이렉션 오류와 실제 Rust 검사 통과를 구분해 `artifacts/rea/result-format-native-probes.json`에 결과를 보존했다.

통합 ignored 로그는 `artifacts/rea/result-format-workspace-windows-tests.log`, `result-format-windows-clippy.log`, `result-format-windows-build.log`, `result-format-peets-check.log`다. 서식 비교·native 예제·최종 schema 증거는 `artifacts/rea/pdf-comparison-20261016/`에 있다. Windows/Linux GUI·원본 런타임·실물 인쇄를 사용하지 않았다. 표·rich text/강조·기수/숫자 분수·round-trip sentinel·전체 문서 기본 서식 변경·다중 선택·전체 formatter 경계·벡터 PDF는 미완료다.

## 최신 검증: 2026-10-16 실제 원본 표·색상·벡터 PDF·단위·정렬점

같은 최종 Rust 소스로 Windows release와 peets에서 workspace **각 430개 통과, 8개 ignored**, fmt·전체 타깃 Clippy(`-D warnings`)·workspace 빌드를 확인했다. 앱 194개, compat 137개, document 53개, editor 33개, core 13개다. peets의 격리 소스는 `/home/peets/Projects/mathcraft-native-output-20261016-r48`이며 target cache만 재사용했다. 두 호스트에 적용한 source tar의 SHA-256은 `110226773270fbc36381d03f143386128d21392378b9cd60ab3a13fd7e82501a`로 일치한다. Windows 실행 파일은 `target/release/mathcraft.exe`, 21,168,128 bytes, SHA-256 `33c96512150cd7a75c02d06b3144a47bfa1911ce4b81ecf47bee6fcebd22d861`다.

실제 사용자 경로의 `1015_LCC_S.mcdx`/`1016_LCC_S.mcdx`를 원본 수정 없이 로컬 조사용 사본으로 검사했다. Windows와 peets에서 각각 cold open→print snapshot→벡터 PDF를 별도 명시 실행했다. 각 문서는 107영역, 저장 결과 64개 복원, 표시 대상 Pending 0개, 수식 실행 0회, A3 2쪽이다. 원본 표의 6열·55개 리터럴·5개 빈칸과 12개 이름/단위 헤더를 복원한다. 독립 PyMuPDF/pypdf 검사에서 4개 PDF 모두 실제 표 영역의 55개 값을 대조하고 마지막 awg 열, `1.00`, 한글·선택 텍스트, 보라/청록/노랑과 흰 글자, 이미지 0개와 벡터 도형을 확인했다. 1015의 94.359%와 64.267∠35°Ω도 유지된다.

| 새 PDF | Windows | peets |
| --- | ---: | ---: |
| 1015, A3 2쪽 | 378,847 bytes | 196,284 bytes |
| 1016, A3 2쪽 | 378,936 bytes | 196,319 bytes |

각 PDF는 벡터 도형 1,117개이며 글자 수는 글꼴별 추출 차이 때문에 2,922–2,934자다. 과거 래스터 Mathcraft PDF는 1,892,095 bytes, 원본 Mathcad PDF는 580,073 bytes였다. 폰트·구성 객체가 다른 독립 출력의 파일 크기와 글자 수를 원본 전체 외형 일치의 근거로 삼지 않는다. Windows는 TrueType subset, Linux Noto CJK는 Type3 벡터 글리프와 ToUnicode로 한글을 보존한다.

추가 Windows 검사에서는 검증된 소스의 ignored 사본에 검사 전용 모듈만 넣어 실제 앱의 열기·자체 저장/재열기·MCDX export/재열기를 실행했다. 두 저장 경로 모두 107영역의 rich document/배경색을 비교해 일치했고 계산은 0회였다. MCDX export 뒤 원본 XamlPackage 27개는 바이트 단위 동일하며 worksheet/header/footer/presentation 4파트는 설치 XSD 검증을 통과했다. 별도 검사 모듈은 제품 소스와 위 430개 회귀 합계에 포함하지 않았다. 증거는 `native-vector-output-20261016/roundtrip-1015-final.log`/`roundtrip-validation.json`이다.

정적 REA로 저장 top의 baseline/표 정렬점 계약을 확인해 [좌표 변환](analysis/0052-native-alignment-and-print-integration.md)을 교정했다. 새 1015 PDF의 첫 제목·Pout P의 문자 origin은 원본 PDF와 가로·세로 모두 0.009 pt 이내다. 이 두 기준점 검사가 모든 수식·표 셀의 간격과 원본 글꼴 외형을 입증하는 것은 아니다. 오른쪽 여백에서 표를 잘라 awg 열을 없애던 오류도 실제 종이 경계로 교정했다.

[단위 조사](analysis/0050-unit-regressions.md)는 앱과 같은 bridge에서 기존 세 문서 172개 행 오류 0·134개 저장 숫자 일치, 재계산 뒤 V/Ω/W 표시 변경 56→0을 확인했다. 추가 1015/1016도 각각 최상위 수식 79개 오류 0·64개 저장 숫자/단위 일치다. Spec Table 내부 6개 열 정의의 계산은 이 성공 범위에 포함하지 않는다. [입력 이력 비용](analysis/0051-typing-history-cost.md)은 4,000영역에서 1,000회 전체 복제 1,415,743 μs 대비 활성 영역 복제 132 μs였으며 전체 프레임/GPU 지연 개선 배수로 해석하지 않는다.

ignored 파일 probe·PDF fixture·CPU benchmark는 workspace 430개와 별도로 실행했다. 전체 8개 ignored 중 일부는 원본 파일/로컬 글꼴이 필요하므로 전체 자동 실행을 했다는 주장은 하지 않는다. 최종 로그는 `artifacts/rea/native-output-{windows-tests,windows-clippy,windows-build,peets-check}.log`, 실제 PDF와 독립 검증은 `artifacts/rea/native-vector-output-20261016/validation.json`, 소스/바이너리 해시는 같은 폴더의 `source-validation.json`이다. 생성 PDF와 source 사본은 Git에서 제외한다.

당시 표 셀 편집·계산 바인딩은 미완료였다. 후속 구현과 검증은 다음 기록으로 구분한다. full rich text 저작, 전체 native 객체·단위계·복합 단위 분해·조판과 페이지 경계, 실제 GPU/IME·클립보드·프린터는 미완료 또는 미검증이다. GUI·스크린샷·원본 런타임을 새로 사용하지 않았다. 테스트 개수를 전체 제품 완성률로 환산하지 않는다.

## 표 셀 편집·열 계산·화면 조판·저장 결과 복원 — 2026-10-16

최신 동일 소스에서 Windows와 peets 각각 workspace **454개 통과·12개 ignored**, fmt·전체 타깃 Clippy(`-D warnings`)·workspace 빌드를 확인했다. 앱 203개, compat 144개, document 60개, editor 34개, core 13개다. Windows 배포 실행 파일은 release 빌드했으며 테스트는 최적화된 dev/test profile이다. 이전 430개 및 중간 452개 검사와 구분한다. 최종 peets 격리 소스는 `/home/peets/Projects/mathcraft-spec-table-20261016-r56`이다.

두 호스트의 source tar SHA-256은 `9298a81b14a2f45acf00fce906b54c460245b8a30018623edc7b79b5963a9510`으로 일치한다. 최종 67개 Rust 파일과 Cargo 설정·lock을 tar와 다시 대조했다. `target/release/mathcraft.exe`는 **21,374,464 bytes**, SHA-256 `90f7010a94478a9f9ccf5cc71a3c1346ba11c99a458d0a2790fc578e9d03d2d7`다. 파일별 해시는 `artifacts/rea/spec-table-integration-20261016/source-validation.json`에 있다.

가져온 Spec Table은 [0053](analysis/0053-spec-table-editing.md)의 원본 단일 클릭·셀 이동 계약을 따라 이름·단위·데이터 셀을 구조 수식으로 편집한다. 키 입력은 초안으로 남기고 Enter·셀 이동·영역 이탈·명시 계산/저장 시 확정한다. Tab/Shift+Tab은 행 순서로 이동하고 마지막 셀 Tab은 빈 행을 추가한다. 원자적 셀 편집, 오류 시 원본 보존, draft Undo와 문서 Undo/Redo, native 표시 캐시 갱신 및 변경 열의 MCDX resultRef 무효화를 검사했다. 복합 수식 안에서 표 셀로 나가는 모든 방향키 전이는 아직 구현하지 않았다.

[0054](analysis/0054-spec-table-calculation.md)는 각 열을 같은 scope의 단위 있는 열벡터 또는 한 값의 스칼라 정의로 연결한다. 기존 이름 재정의 오류가 과거 값을 되살리지 않는 경우, 위치 순서·문맥 라벨·표 비활성화·checkpoint 재생·표 내부 협력 취소를 검사했다. 실제 1015/1016은 각각 수식 79개와 표 1개, 80개 외부 계산 행의 오류가 0이다. 별도 downstream query로 6열 길이 10/10/10/10/8/7 및 숫자 55개·물리 단위를 비교했다. 원본 native evaluator의 마지막 padding 제거 경로는 미확인이라 독립 정규화와 구분한다.

양 OS에서 두 실제 문서 각각 첫 표 값을 42 A로 수정하고 계산→자체 저장/재열기→MCDX export/재열기를 실행했다. 107영역·6열·10행과 나머지 셀을 보존하며, 자체 저장 재열기에서는 **표시 결과 64개·Pending 0개·실행 0회**다. 입력 표의 원본 열 cache가 Synchronized이고 숫자 payload가 없어도 정상 입력 상태로 복원한다. 현재 XML이 원본과 정확히 같을 때만 이 상태를 가져온다. MCDX의 Pending cache는 새 원본 실행 결과로 취급하지 않는다.

이 과정에서 [JSON 숫자 정밀도 결함](analysis/0056-native-cache-float-roundtrip.md)을 발견했다. 기본 JSON 파서가 native 좌표를 1 ULP 바꿔 저장 캐시의 소스 서명이 달라졌다. `float_roundtrip`으로 정확한 f64 복원을 사용하며 서명 검증·migration을 완화하지 않았다. 실제 좌표 5개의 비트·서명·저장 cache 회귀는 수정 전 실패, 수정 후 통과했다.

수정된 MCDX 4개는 설치 XSD 16개 part 검사를 통과했고 문서마다 XamlPackage 27개가 원본 바이트와 일치한다. 양 OS에서 원본 1015/1016과 수정 후 자체 저장 1015를 새 벡터 PDF로 만들었다. 독립 PDF 객체/텍스트 검사에서 6개 모두 A3 2쪽·이미지 0개·검색 가능한 글자·실제 표 숫자 55개를 확인했고, 수정된 셀은 42로 출력된다. 원본 4개는 색상과 두 기준점의 기존 대조도 통과했다. 화면 이미지나 스크린샷은 생성·검사하지 않았다.

[0055](analysis/0055-screen-rendering-cost.md)는 확대 시 글자 atlas 밀도를 OS DPI×확대율에 맞추고, 반복 AST 전체 비교를 O(1) revision 검사로 바꾸며 hit geometry와 readonly 조판을 재사용한다. 4,097노드 수식의 반복 cache 조회 2,000회는 69.180→0.118 ms, interactive 배치 120회는 92.903→42.253 ms였다. 전체 4,000영역 idle은 144.817→135.421 ms, pointer hold는 148.791→171.621 ms로 결과가 엇갈렸다. 전체 앱 개선 배수로 제시하지 않는다. 최종 실제 1015의 CPU-only 안정화 120 frames는 DPI/zoom 4조건에서 32.566–58.733 ms, 계산 0회다. GPU 표시·입력 지연의 측정은 아니다.

최종 로그는 `artifacts/rea/spec-table-final-{windows-tests,windows-clippy,windows-build,peets-check}.log`, 실제 편집/재열기는 `spec-table-final-integration-{windows,peets}.log`, PDF/스키마 대조는 `spec-table-integration-20261016/{pdf-verified/validation.json,schema-and-table-validation.json}`이다. GUI·스크린샷·원본 런타임·실제 프린터는 사용하지 않았다. 새 표 생성, 전체 행/열 명령과 다중 셀 선택, 모든 셀 오류 위치·편집 전이, 전체 rich text·단위·조판과 실제 화면의 체감 지연은 남아 있다.

## 원본 단축키·구조 수식·표 명령·입력 지연 — 2026-10-16

최종 동일 소스의 Windows와 peets workspace는 **각 494개 통과·15개 ignored·실패 0개**다. 앱 234개, compat 146개, document 67개, editor 34개, core 13개다. fmt·전체 타깃 Clippy(`-D warnings`)와 workspace 빌드가 통과했다. Windows 실행 파일은 release, 테스트는 최적화된 dev/test profile이다. 소스를 변경하지 않은 같은 38개 원본 참조 표본의 좌표 두 경로도 양 OS CLI에서 모두 일치했다.

새 peets 디렉터리는 `/home/peets/Projects/mathcraft-original-workflow-20261016-r60`, source tar SHA-256은 `ae6af1938931635083ebd52c1b008ba6b3d7ef5846196d4d3c0bf0668cc64a93`로 양 호스트가 일치한다. Rust 70파일과 Cargo 설정·lock을 현재 소스와 다시 대조했다. `target/release/mathcraft.exe`는 **21,533,696 bytes**, SHA-256 `9760fa9454f3484837b9452368de2573be0ff172ce01a1e6a9dc02abea270eba`다. 파일별 해시는 ignored `artifacts/rea/original-workflow-integration-20261016/source-validation.json`에 보존한다.

[0057](analysis/0057-shortcut-and-subscript-contract.md)의 Ctrl+- 이름 아래첨자, 비교 6종·논리 3종·NOT, 무한대와 원본 키 명령을 구조 AST·MathML·조판·계산에 연결했다. global→history→실제 수식 key 이벤트 회귀는 Ctrl+-가 배율을 바꾸지 않고, Ctrl+Shift+Z가 Undo를 하지 않으며, Ctrl+Shift+2가 도표 대신 OR를 만드는 것을 검사한다. 추가 Shift/Alt를 전역 키 소비가 무시하던 결함을 교정했다. Ctrl+F1 리본 그룹 숨김/복원, F1 안내, F5 가시 계산과 Ctrl+F5 전체 계산도 구분한다. 이름 중간의 임의 rich-run 깊이와 텍스트 위/아래첨자 저작 전체는 미완료다.

[0059](analysis/0059-spec-table-structural-commands.md)의 원본 생성·구조 명령을 앱에도 연결했다. 표 focused 회귀 14개는 새 1×1 데이터 표, 문서 서식 상속, 행/열 삽입·삭제의 caret 경계, 최소 슬롯 비우기, 셀 draft+구조 변경 한 번의 history/계산, 편집 중 Undo/Redo, 사각 범위 Copy/Delete와 body matrix paste, 실제 클릭 확정 history, 배율 2+평행 이동의 선택 좌표와 Ribbon Copy 보존, 이름/첨자 끝 셀 이동을 검사한다. 다른 영역을 선택하면 이전 표의 범위를 해제한다. Cut·원본 전체 clipboard 형식·붙여넣기 자동 확장·헤더/단위 matrix paste·복합식의 모든 셀 경계 전이는 미완료다.

수식 리본의 index는 기존 피연산자를 보존하며, 표 셀에 단위·이름·첨자·구조 수식을 넣을 때 새로운 수식 영역을 만들지 않는다. 기존 아래첨자의 caret만 전환하면 문서 history나 계산 대기를 만들지 않는다. 새 표의 폭은 원본 측정값이 없는 경우 자연 셀 폭을 사용하며 기존 가져온 표의 실제 폭과 구분한다. 빈 표와 채운 표의 새 MCDX 2개는 worksheet/header/footer/presentation **8개 XML part**의 설치 XSD 검사를 통과했다.

[0058](analysis/0058-input-and-calculation-latency.md)의 O(N²) 정의 복제와 반복 계산 준비를 줄였다. 4천 정의 cold 준비는 984→110ms, 동일 입력 retain은 6.2ms였다. 드래그 회귀 5개는 중간 source/dirty/history/worker 제출 없음, drop 한 번 확정, 확대 좌표, 마지막 release 위치, 같은 frame의 빠른 press→move, Undo/Redo의 다른 영역 보존을 검사한다. 4천 영역의 60 delta는 평균 **1.60ms/frame**, drop은 **73.60ms**였다. 아직 남은 큰 문서의 확정/첫 준비 비용과 실제 GPU·IME·모니터의 체감 지연을 숨기지 않는다.

검증 로그는 `artifacts/rea/original-workflow-{windows-tests,windows-clippy,windows-build,peets-check}-20261016.log`다. 새 원본 실행·GUI·스크린샷·실제 프린터는 사용하지 않았다. [이번 작성 흐름과 20% 목표의 경계](analysis/0060-original-workflow-integration.md)를 따르며 테스트 개수를 전체 완성률로 환산하지 않는다.

최종 실제 문서 회귀도 양 OS에서 다시 실행했다. 1015/1016 각각 첫 표 값을 42 A로 수정한 뒤 계산 80행·오류 0개, 자체 저장 재열기 64개 표시·Pending 0개·실행 0회를 확인했다. MCDX export/reopen은 셀·단위를 보존하며 원본 XamlPackage 27개도 바이트 단위 동일하다. 수정 MCDX 4개는 설치 XSD 16개 part를 통과했고 신규 빈/채운 표 2개까지 합하면 **6개 MCDX·24개 XML part**다.

원본 1015/1016과 수정된 자체 저장 1015를 양 OS에서 출력한 최종 벡터 PDF 6개는 모두 A3 2쪽·이미지 0개·표 숫자 55개·복원 표시 64개이며, 수정된 값 42도 확인했다. 원본 출력의 색상과 기준점 대조도 통과했다. 편집 MCDX의 결과 cache는 export 시 Pending으로 무효화하므로, 해당 MCDX를 계산 없이 다시 연 PDF와 결과 cache를 가진 `.mathcraft` 출력은 구분한다. 이를 원본 실행 없이 Synchronized로 조작하지 않는다. 새 증거는 `artifacts/rea/original-workflow-integration-20261016/{schema-and-table-validation.json,pdf-verified/validation.json,new-table-schema/validation.json}`에 있다. GUI·스크린샷·원본 실행은 추가하지 않았다.

## 배열 원소 정의·표 clipboard 교정·계산 준비 2차 — 2026-10-16

동일 r64 소스로 Windows와 peets 각각 **526개 통과·실패 0개·18개 ignored**, fmt·전체 타깃 Clippy(`-D warnings`)·workspace 빌드가 통과했다. 앱 248개, compat 161개, document 70개, editor 34개, core 13개다. Windows 실행 파일은 release이며 테스트는 최적화된 dev/test profile이다. 양 OS CLI에서 기존 원본 참조 사례의 saved/input 좌표 각각 38/38, 차이 0을 다시 확인했다.

peets의 새 격리 소스는 `/home/peets/Projects/mathcraft-indexed-workflow-20261016-r64`, source tar SHA-256은 `56ad07499aeea659d8ff3b4b1e95c8ba8467ea357e93b83d0ec4e047a4f7f67d`다. 양 호스트의 archive SHA가 일치하며 Rust 73개 파일과 Cargo 설정·lock을 현재 파일과 재대조했다. `target/release/mathcraft.exe`는 **21,626,368 bytes**, SHA-256 `707036f81f27e4f140d385ce7bb2751b1a12f31ee6ec4509dce299ed249a0247`다. 증거 manifest는 `artifacts/rea/indexed-workflow-integration-20261016/source-validation.json`이다.

[0063](analysis/0063-indexed-definition-contract.md)의 배열 원소 정의는 1/2 인덱스, 범위 변수·대각선, 새 배열의 확장·0 채움, ORIGIN, 단위·복소수, 기존 값·alias·captured scope·checkpoint, 실패한 정의와 취소 경계를 연결했다. SI 이름 A/m/V의 정의 LHS와 명시 UNIT RHS도 구분한다. 단위 있는 배열의 빈칸은 최초 값의 차원을 보존한다. 행벡터 단일-index 할당은 명시 설치 도움말 예제를 따른다. 저수준 setter와 상위 wrapper의 미확인 부분을 구분하며, 일반 행벡터 단일-index 조회·문자열/중첩 배열 원소·자기참조 범위 recurrence·fill metadata 없는 혼합 단위 배열 확장은 미완료다. 엔진 집중 회귀 11개와 실제 한 글자 키 입력→MathML→계산·Undo/Redo 회귀를 검사했다.

추가 파일 probe는 양 OS에서 7개 영역의 열벡터·2-index 행렬·복소 행벡터 갱신을 계산하고 자체 저장/재열기 및 새 MCDX export/import/명시 재계산까지 수행했다. 쿼리 표시 3개, 자체 재열기 실행 0회·Pending 없음, native 재계산 오류 0개이며 값·크기·물리 차원을 비교한다. native XML에는 `v`, `M`, `r`의 실제 `apply(indexer,...)` 정의 세 개가 남아 있다. 이 검사는 새 Mathcad 실행과 구분한다.

[0061](analysis/0061-spec-table-clipboard-contract.md)/[0064](analysis/0064-table-clipboard-application.md)에서 기존 표 clipboard 해석을 교정했다. 원본 Copy의 caller를 확인한 결과 본문만은 행렬, 이름/단위 포함은 새 표 객체다. Cut은 복사 후 원본 삭제/최소 슬롯 규칙, Clear Cells는 크기·서식을 유지한 내용 삭제다. 원본 근거가 없는 body 직사각형 분배 Paste 경로는 제거했으며, 활성 셀의 구조 수식 Paste로 연결했다. 잘못된 새 표 payload는 기존 셀 초안을 먼저 확정하지 않는다. 새 표 객체는 대상 문서 기본 서식을 사용하며 전체 영역 Copy는 기존 영역 서식을 보존한다. 새 ID·native visual·Undo/Redo·삭제 cache 해제도 검사했다. 표 집중 회귀는 19개, 별도 영역/명령 회귀는 6개가 통과했다. native OS clipboard binary format·정확한 모든 focus 전이·일반 행렬의 다중 셀 Clear는 미완료다.

[0062](analysis/0062-input-preparation-second-pass.md)는 표시 tree 복원과 global prepass의 중복 XML parse를 제거하고, 정확한 tree 값으로 직렬화를 재사용한다. raw XML·문맥 순서·설정·archive 바이트가 모두 같을 때만 좌표를 갱신한다. 같은 phase 계측에서 4천 scalar cold 준비는 **80.3→54.6ms**, 순서 유지 배치 준비는 **9.4ms**다. 실제 수평 드래그 drop은 **12.6ms**, 순서가 바뀌어 전체 문맥을 다시 준비하는 drop은 **73.6→44.0ms**였다. 순서 변경을 cache hit로 강제하거나 contextual label을 지우지 않는다. 추가 cache의 메모리 비용, 큰 내용 변경의 전체 준비와 실제 GPU/IME 체감 미검증을 명시한다.

기존 실제 1015/1016도 양 OS에서 다시 수정·계산·자체 저장/재열기·MCDX export/import해 문서마다 80행·오류 0개·수정 값 42 A·복원 표시 64개를 확인했다. XamlPackage 27개는 각 원본과 동일하다. 두 원본과 수정된 자체 저장 1015의 양 OS 벡터 PDF 6개는 A3 2쪽·이미지 0개·표 숫자 55개와 수정 값·64개 표시를 유지한다. 색상·기준점 검사도 통과했다. 수정 MCDX 4개, 신규/복사 표 5개, 인덱스 정의 MCDX 2개는 총 **11개 MCDX·44개 XML part**의 설치 XSD 검사를 통과했다.

로그는 `artifacts/rea/indexed-workflow-{windows-tests,windows-clippy,windows-build,peets-check,windows-reference,peets-reference}-20261016.log`, 별도 파일 검사는 `indexed-workflow-{windows-probes,peets-probes}-20261016.log`다. PDF/스키마 대조는 `indexed-workflow-integration-20261016/{pdf-verified/validation.json,schema-and-table-validation.json,new-operations-schema-validation.json}`에 보존한다. 일반 workspace에서 ignored 18개 전부를 자동 실행했다고 주장하지 않는다. GUI·스크린샷·원본 런타임·실제 프린터는 사용하지 않았으며 이 검증을 전체 제품 20% 달성으로 환산하지 않는다.
