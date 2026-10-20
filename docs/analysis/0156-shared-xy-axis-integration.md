# 공유 XY 축의 독립 문서 모델과 native 연결

2026-10-19. 사용자 평가15%에서40% 목표를 계속 진행하는 후속 작업이다. 원본 근거는 [0151](0151-native-xy-axis-pairing-contract.md), 설계와 평가 횟수의 미확정 범위는 [0155](0155-shared-xy-axis-model-design.md)에 기록했다. GUI·스크린샷·원본 runtime을 사용하지 않았다.

r156 같은 source의 Windows/peets 전체 검증을 완료했다. 축 순서와 짧은 목록의 마지막 요소 공유, owner 내부 equation ID, native 재출력의 목록 길이를 보존한다. 기존 legacy JSON/cache signature는 이전 serializer oracle과 대조한다. Shift+Enter·중간 삽입/삭제·원본 전체 EquationControl은 아직 미완료다. 이 변경이나 검사 수를40% 달성으로 환산하지 않는다.

## 문서 권위와 이전 파일 호환

PlotSpec은 source enum 하나만 편집 권위로 가진다. Legacy는 이전 단일 식/독립 pair의 JSON을 custom serde로 그대로 읽고 쓴다. 이전7필드 순서, inactive fallback 문자열까지 유지하여 기존 계산 signature를 바꾸지 않는다. AxisLists는 x/y의 ordered equation {id,source}와 appearances, next_equation_id를 보관한다. 같은 문자열인 서로 다른 식을 자동으로 합치지 않는다. 명시적 migration도 각 슬롯에 별도 ID를 부여한다. ID는 독립 앱의 owner-local identity이며 원본 엔진 handle을 복구했다는 뜻이 아니다.

새 source와 legacy 필드 혼합, 중복 JSON key/ID, next ID 역전/overflow, 빈 축 목록, appearance 개수 불일치, 예산 초과를 거절한다. 내부 primary accessor는 검증된 nonempty 목록을 전제로 한다. source는 현재 문자열 subset이며 원본 구조 EquationControl 전체의 대체 완료가 아니다. 현재16식 상한은 앱 정책이고 원본 cap 증거가 아니다.

## 공유 편집과 native 왕복

trace i는 X[min(i,nx−1)]/Y[min(i,ny−1)]를 참조한다. primary 식 편집은 해당 슬롯 하나를 바꾸며 실제 연결된 모든 curve가 다음 계산에서 반영된다. modal 설정에서도 공유 목록은 각 식을 한 번씩 편집한다. 공유 목록에 legacy trace 추가/삭제를 적용해 독립 pair로 펼치는 경로는 거절한다. 일반 inline 화면의 다중 식 슬롯 직접 편집·Shift+Enter는 다음 단계다. 인쇄는 authored 축 목록을 표시하여 하나의 공유 식을 trace 수만큼 반복하지 않는다.

native reader는 독립 x/y 개수를 읽고 trace 수=max(nx,ny)를 요구한다. 불균등 목록은 AxisLists로 가져와 길이와 순서를 그대로 다시 쓴다. 기존 equal N/N import는 Legacy wire/cache 호환을 유지한다. same_source는 계산상 expanded pair가 같더라도1/N과N/N을 구분한다. owner-local ID만 다른 것은 native source 차이로 취급하지 않는다. resultRef 순서와 기존 curve cache의 엄격한 소유권 검증은 유지한다.

## 계산과 검증 경계

공유 source 도입은 평가 횟수 최적화와 분리했다. evaluator는 각 resolved trace를 기존 방식으로 계산한다. 원본이 같은 식을 한 번만 평가한다는 증거가 없으므로 난수·상태 의존 함수의 횟수를 문자열 memoization으로 바꾸지 않았다. native/own cold open과 print는 저장 curve를 읽으며 새 계산을 실행하지 않는다.

model oracle은 이전 derive serializer+SHA 및 실제 cache 재열기, 같은 문자열의 독립 ID, min-index 공유, malformed wire를 검증한다. native 회귀는1/3·3/1·2/3·3/2·15/16 source shape, ordinal refs/styles, 공유 primary 수정, 빈 축/trace mismatch 거절을 확인한다. 앱 통합은 실제 Text/Tab/Enter 이벤트, 미확정 원문/curve 불변, Manual Pending, 단일 history, Undo/Redo, own ID 보존, 계산값, native cache 그대로 복사, native cold 계산0과 vector print 계산0을 X/Y 양방향으로 확인한다. 정적 REA에서 도출한 독립 fixture를 사용했으며 원본 runtime 관찰이라고 하지 않는다.

## 같은 source의 양 OS 검증

Windows workspace1293+backend2, peets1295가 통과했다. 실패0/ignored67, fmt·workspace/backend Clippy·build·reference38saved/38input·Windows release를 확인했다. 양 OS 합산38MCDX/190XML parts의 XSD 오류0,38PDF의 이미지 객체0과 fitz/pypdf 두 reader 추출을 확인했다. 새 shared-X/Y probe의 native cold open·cache 그대로 copy·실제 입력·Undo/Redo·own/native/PDF를 포함한다. 전체 Mathcad 기능이나 GUI/실물 인쇄 검증으로 확대하지 않는다.

source archive `artifacts/rea/mathcraft-shared-plot-20261019-r156.tar`는11,909,120bytes SHA `4218a5fa4aea63ec69b21fc6c4f04d7b6778549a22f07f302829c5b142dd86df`, manifest SHA `b8b576faff262d82dc989c720f3d5b52445bda485f43f26824404be47fd75c7c`다.415files/197product Rust를 고정했고 `/home/peets/Projects/mathcraft-shared-plot-20261019-r156`에서 모든 source hash를 검증했다. 회수 evidence2,048,000bytes SHA `74dba9692b219d3507146157ee57d1e6be7cc845261f058e13111c8f70d5fa90`는 양끝 일치한다. 최종 `artifacts/rea/shared-plot-integration-20261019-r156/validation.json`은 current product=archive를 확인했다. archive에는 준비 상태 문서가 들어 있으며 후속 문서 갱신은 고정 제품 소스를 바꾸지 않는다.

release23,508,992bytes SHA `091d7f8d6c88cce91ebb53c4721c1408449c78ff8b7c8de3e682c4d0b579bae6`, CPU harness24,815,104bytes SHA `feff609727164a23d58b97ee6a90cbdd5d411fd6671a9a13f262445f666d6e71`다. release는 r154보다161,280bytes 증가했다. 기능/호환/CPU 입력 흐름을 검증했으며 전체 GPU·앱 지연의 새 절대 수치를 측정한 것은 아니다. [0157](0157-native-xy-slot-authoring-contract.md)의 슬롯 삽입/삭제 계약을 후속으로 진행한다.
