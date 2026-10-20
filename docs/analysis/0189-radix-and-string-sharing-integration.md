# 0189 — 진법 입력과 런타임 String 공유 통합

2026-10-19. r203 고정 검증은 완료했다. Windows/peets 각각1582검사와 128MCDX·별도10XML의 XSD 778parts·62PDF 구조 검증을 통과했다. 승인된 직전 기준은 [r202](0186-string-cache-and-plot-source.md)이다. 고정 archive에는 검증 전 문서가 포함되며, 이 후속 기록은 완료 로그와 연결한다. 기능 행 수와 테스트 개수를 제품 완성률로 환산하지 않는다.

## 이번 연결 범위

[원본 계약](0188-radix-input-contract.md)을 바탕으로 `BasedNumber`는 suffix를 포함한 전체 원문을 보유한다. base와 원문의 일치를 검증하고, native real의 base 속성을 reader/writer·식 eligibility·plot source·화면 조판으로 전달한다. 명시적 원문은 현재 입력 모드와 독립적이다. 알 수 없는 메타데이터를 지우고 계산 가능한 숫자로 바꾸지 않는다.

리본 계산 옵션의 Bin/Oct/Hex 모드는 직접 calculationBehavior 속성을 읽고 쓴다. own 문서의 선택적 override는 이전 wire에 없을 때 생략한다. 모드 변경·Undo/Redo는 기존 AST·계산 서명·결과·실행 횟수를 바꾸지 않는다. 이후 입력만 새로운 모드를 따른다. 설정 part의 relocated path·UTF16·기타 옵션·스키마 순서를 유지하고, ZIP 파일이 없어도 canonical PartName이 이미 다른 타입에 예약돼 있으면 충돌을 거절한다.

실제 Text/Enter 흐름의 `1101b`, `32o`, `0a32h`를 구조 수식으로 작성하고, 수동 Enter는 이전 답을 유지한 채 확정한다. Calculate Now는 각각13·26·2610을 계산한다. native/own 저장 후 cold open은 실행0회로 source·설정·Sync 결과를 복원하고, native 복사 저장의 result.xml bytes를 보존한다. 모드 OFF 전환 후에도 이미 명시된 진법 source와 값은 재해석하지 않는다. 이 흐름을 벡터 PDF까지 연결한다.

`2b` 뒤의3/c를 잃던 오류는 확인된 suffix 분리 규칙으로 고친다. 원본0491은 모드와 독립적이므로 가져온 명시적 source의 모드 OFF도 같은 제한된 분리를 따른다. root/Evaluate·숫자 prefix·Normal 끝 캐럿만 연결하며, 이 변화는 하나의 Undo 명령이다. `0a32h`/`2b3ch` 직접 Scale fusion은 독립 지원 경로이며 전체 키별 native 동일성은 아직 미확정이다.

계산 decoder는 unsigned ASCII 정수와2/8/16·값 최대2^53·source64KiB의 명시적 독립 경계를 둔다. fraction·정밀도/오버플로 원본 알고리즘 전체는 복원하지 않았다. 캐시의 real@base를 십진으로 잘못 읽던 경로도 공유 decoder로 고쳤다. source literal의16KiB 원자 한도와 decoder 한도는 서로 다르다. StringLiteral도 기존 문서 총8MiB source text 합산에 포함하도록 누락을 고쳤다.

런타임 String 결과만 불변 Arc로 공유한다. source/editor String과 legacy Value JSON은 유지한다. [0187](0187-runtime-string-sharing.md)에 관찰자 없는 비교·별도 할당 계수·기각 빌드·생성/역직렬화 악화가 모두 있다. 256KiB/32조회 표본에서 최초 계산8.49→2.84ms, 실행0회 snapshot3.15→0.015ms였다. 앱 전체 체감·GPU·Linux 속도 측정은 아니다.

## 검증과 남은 범위

- 최초 [typing 실패](../../artifacts/rea/radix-input-initial-red-20261019.log)는 fixture 캐럿 오류와 문자가 사라지는 실제 오류를 함께 보존한다. fixture의 tail 위치만 고쳤고 요구 사례는 삭제하지 않았다.
- [분리 후 입력7검사](../../artifacts/rea/radix-input-split-preflight-20261019.log)는 통과했다. 이후 mode OFF·Eval ID·Undo/Redo 회귀를 추가했다.
- 검토에서 발견한 Eval 반대쪽의 전체 트리 한도 누락도 고쳤다. [최종 입력9검사](../../artifacts/rea/radix-input-budget-preflight-20261019.log)는16,383노드의 정상 source에 두 노드를 더하는 입력을 변경 전에 거절하고 원문·캐럿·revision·Redo를 보존하는 회귀를 포함한다.
- 첫 [Enter 통합 실패](../../artifacts/rea/radix-input-flow-preflight-20261019.log)는 수동 모드에서 Enter 즉시 계산을 기대한 잘못된 oracle이었다. 제품 계산 정책을 바꾸지 않고 이전 답·Pending·명시 계산을 검사한 [수정 통합](../../artifacts/rea/radix-input-flow-preflight2-20261019.log)은 통과했다.
- [workspace all-target Clippy](../../artifacts/rea/radix-sharing-clippy-preflight-20261019.log)는 경고0 통과했다. 이 검사는 고정 소스의 최종 Windows/peets 게이트와 구분한다.

전체 native navigator·partial/body/delete·IME·진법 fraction·정밀도·타 진법 형식 지정·원본 키별 비교·전체 결과 UI 크기 제한·도표 marker CPU는 남아 있다. GUI·스크린샷·설치 Mathcad/CLR/COM 실행은 사용하지 않았다.

## 고정 소스의 최종 게이트

[최종 검증 JSON](../../artifacts/rea/string-values-integration-20261019-r203/validation.json)과 [소스 manifest](../../artifacts/rea/string-values-integration-20261019-r203/source-manifest.json)를 기준으로 채택했다. 소스516파일·제품 Rust265파일, archive SHA256 `bfa25c2c738e59e6e00c84769183272344d308c549084a70eebdc8e6459bf462`, manifest SHA256 `3419ab29dbdc71f63843496a7daadfcb858e62b2d58af3e9004258b6af74a31f`다.

Windows workspace 1580 + backend 2 = 1582, peets 1582; ignored 84는 일반 성공 수에 포함하지 않는다. fmt·workspace/backend Clippy 경고0·build·reference·release와 명시 probe가 통과했다. XML/PDF는 예상 경로 전체 집합과 일치하며 둘 다 OS별 수가 같다. 이미지 객체0과 두 parser 텍스트 검증은 외형 동일성 검증이 아니다.

| 산출물 | 바이트 | SHA256 |
|---|---:|---|
| [mathcraft-string-values-r203-20261019.exe](../../artifacts/rea/mathcraft-string-values-r203-20261019.exe) | 24925184 | `d504665a373c8e29b640ca25353da78c1b7c2e7a40fe9edf0a46daf783d2d30e` |
| [string-values-r203-headless-20261019.exe](../../artifacts/rea/string-values-r203-headless-20261019.exe) | 27841536 | `cb6692278c6536c0b1ff25a5054fb4e1c210b47ae9cbb99e9c2068e5036a071f` |
| [string-values-r203-peets-evidence.tar](../../artifacts/rea/string-values-r203-peets-evidence.tar) | 5263360 | `04b46e3be074b7c29222477d425220b504fb6988800414d65deeef852dec0fe0` |

이 기준점은 진법 부분 지원과 String 소유권 통합을 검증한 것이며40% 목표 달성을 판정하지 않는다.
