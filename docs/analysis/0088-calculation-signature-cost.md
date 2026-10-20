# 계산 입력 서명의 전체 JSON 재직렬화 비용

2026-10-19. [0084](0084-prepared-source-cost.md) 이후에도 입력 준비마다 모든 활성 영역을 다시 JSON으로 만들고 SHA-256을 계산했다. 이번 변경은 계산 결과·문맥 라벨이 아니라 **정확한 영역 snapshot의 직렬화**를 보존한다. `CanvasDocument::active_calculation_signature()`의 기존 v1 결과와 바이트 단위 source 형식을 유지한다. GUI·스크린샷·원본 런타임은 사용하지 않았다.

## 읽기 전용 후보 비교

먼저 제품 파일을 수정하지 않는 별도 Rust probe로 기존 공개 API와 후보를 비교했다. 15회 반복 중 원래 방식과 memo 후보의 선행 순서를 번갈아 실행했다. 같은 serde 구조·도메인·SHA-256·native archive를 사용하며, JSON 전체 바이트와 signature를 비교했다. 별도 probe는 제품 앱이나 원본 Mathcad를 실행하지 않는다.

| 서명 단계, median | 4,000 scalar | 16×16 행렬 100개 |
|---|---:|---:|
| 기존 API | 4.858 ms | 4.527 ms |
| serde를 SHA writer로 직접 전달 | 4.605 ms | 4.616 ms |
| 8 KiB buffered SHA writer | 4.903 ms | 4.913 ms |
| 재사용 Vec에 JSON 생성 후 hash | 4.159 ms | 4.272 ms |
| 정확한 region JSON memo, 하나 수정 | 1.325 ms | 0.842 ms |
| region JSON memo 최초 준비 | 7.382 ms | 7.397 ms |

단순 streaming만으로는 큰 개선이 없었다. 4,000 scalar에서 JSON 생성만 median 4.272 ms, 이미 만든 바이트의 SHA는 0.541 ms였다. 전체 tree snapshot clone은 1.092 ms, 같은 tree의 완전한 equality는 0.081 ms, 기존 XML tuple 전체 clone은 0.381 ms였다. 이 primitive 비용은 서로 독립 측정이며 전체 입력 시간에 그대로 더하거나 뺄 수 없다.

region memo는 warm 경로를 크게 줄이는 대신 cold clone·payload 비용을 추가한다. 최초 clone을 피하면서 정확한 source equality를 유지하려면 기존 MathML 직렬화 snapshot과 소유권을 공유하도록 더 큰 재배치가 필요하다. 이번에는 기존 직렬화·문맥 cache를 섞지 않고 비용이 측정된 독립 cache를 선택했다.

## 구현과 정확성 경계

앱의 private `signature_cache.rs`는 활성 영역별 JSON과 그 전체 `CanvasRegion` snapshot을 보존한다. 현재 벡터 위치는 lookup slot일 뿐 identity가 아니다. ID·내용·원본 region 참조·서식·계산 override를 포함한 snapshot 전체를 비교한다. region ID 재사용이나 순서 변경도 다른 source를 같은 source로 취급하지 않는다.

좌표와 `font_points`의 -0/+0은 `PartialEq`로 같아도 JSON에서는 다르므로 IEEE 비트를 추가 비교한다. tree의 atom·metadata는 문자열·정수·불리언이며 전체 equality를 사용한다. NaN은 equality를 통과하지 않고 원래 serde 경로로 돌아가 기존 null 직렬화 결과를 유지한다. Plot은 중첩 float 필드가 많아 cache에 보존하지 않고 매번 기존 serde로 처리한다. 기존 값 유효성 검사나 문서 저장 허용 범위를 넓힌 변경은 아니다.

최종 입력의 배열 순서와 모든 쉼표·wrapper·settings JSON은 기존 tuple 직렬화와 같다. width/height/grid는 매번 직렬화하고 native archive는 매번 **모든 바이트**를 hash한다. 동일 크기 archive 교체, 서로 다른 NaN payload, 변경되지 않은 pointer 등을 cache identity로 사용하지 않는다. 비활성화 영역·Text는 기존 active signature와 같이 제외하고 해당 snapshot을 정리한다. 삭제·축소·문서 초기화도 보존 항목을 정리한다.

`CanvasDocument`의 공개 저장 서명 API와 버전 문자열은 변경하지 않았다. 앱의 계산 입력 준비 경로만 새 private cache를 사용한다. 문서 저장·캐시 재열기의 기존 signature 계약은 그대로이며, 범용 문서 API를 바꾸거나 weak hash로 대체한 것이 아니다. app에는 document에서 이미 쓰던 `sha2 = 0.10` 직접 의존을 선언했다. lock에는 app 연결만 추가되고 신규 패키지·버전 갱신은 없다.

## 예산과 cold 비용

보존 payload 한도는 **32 MiB**다. index capacity, Entry 구조, JSON capacity와 clone이 소유하는 문자열/tree 벡터 payload를 산정한다. 한도에 맞지 않는 영역은 snapshot을 보존하지 않고 같은 serde 결과를 바로 hash한다. 너무 많은 region이나 매우 작은 예산에서는 전체 기존 직렬화로 돌아간다. 변경·삭제·disable·Text 전환 시 이전 항목을 제거하고 공간이 돌아오면 다시 보존한다. 이 fallback은 계산 결과나 오류를 바꾸지 않는다.

실제 preparation probe에서 추가 보존 payload는 4,000 scalar **4,065,780 bytes**, 16×16 행렬 100개 **4,416,491 bytes**였다. allocator overhead, 임시 JSON, 기존 MathML/geometry/display cache, archive, 프로세스 나머지 메모리는 이 숫자에 포함하지 않는다. **RSS 측정이나 전체 메모리 32 MiB 제한이 아니다.**

## 제품 경로 전후 측정

이전 test harness와 새 test harness를 같은 PC에서 순서를 바꾸며 각각 5회 실행한 median이다. Cargo test profile의 CPU headless 측정이며 자체 코드 opt-level 1·dependency opt-level 2다. 사용자 GUI/GPU 체감이나 다른 PC의 성능을 보장하지 않는다. 실행 파일 SHA와 모든 표본은 JSON에 기록했다.

| 경로, median | 이전 | 변경 후 |
|---|---:|---:|
| 4,000 scalar 수정 후 준비 | 16.885 ms | 12.744 ms |
| 위 준비 중 signature 단계 | 5.653 ms | 1.899 ms |
| 4,000 scalar 변경 없는 준비 | 5.890 ms | 2.139 ms |
| 4,000 scalar 확정·worker 제출 | 23.871 ms | 21.798 ms |
| 4,000 scalar cold retain | 64.634 ms | 69.770 ms |
| 4,000 scalar 실제 draft key frame | 0.983 ms | 1.061 ms |
| 4,000 scalar 전체 재계산 제출 | 7.137 ms | 3.483 ms |
| 16×16 행렬 100개 수정 후 준비 | 6.501 ms | 2.496 ms |
| 16×16 행렬 100개 확정·worker 제출 | 12.067 ms | 8.548 ms |
| 16×16 행렬 100개 cold retain | 42.575 ms | 47.425 ms |
| 실제 1015 107영역 확정·worker 제출 | 1.381 ms | 1.176 ms |

cold signature 단계 자체는 4,000 scalar 5.776→8.098 ms였다. **첫 준비는 더 빨라졌다고 주장하지 않는다.** 위 cold retain 전체에서는 다른 준비 단계·할당·실행 시점의 영향도 함께 보이므로 그 차이를 전부 순수 snapshot clone 비용으로 해석하지 않는다.

확정 후 worker 잔여 시간은 4,000 scalar 30.395→28.019 ms였다. 위 확정·제출은 전체 계산 완료 시간이 아니다. 새 signature cache 이후에도 기존 XML 생성·문맥 라벨·display 준비와 worker 계산 비용이 남는다. 전체 지연 해결이나 제품 완성률을 이 측정으로 환산하지 않는다.

## 차등 검사와 산출물

새 캐시 단위 검사 5개는 생성 JSON **전체 바이트**와 기존 공개 signature를 독립 oracle로 비교한다. 순서·삭제·새 ID, 한글/µ/escape 문자의 identifier·opaque XML, source region·서식·override, disable·Text, 동일 길이 native archive 교체, -0/+0·NaN·±Infinity·settings, Plot 내부 float, cache hit의 직렬화 횟수, 예산 초과·퇴출·회복·region 개수 fallback을 포함한다. source 계약과 계산 입력 서명이 다른 경우가 없도록 검사한다.

추가 앱 검사 1개는 MathML이 전혀 바뀌지 않아도 서식·source reference·settings·archive·override 변경이 signature에는 정확히 반영되는지 확인한다. 기존 input latency 차등 검사에는 기존 문서 API의 signature equality도 추가했다. 관련 `edit_commit` **17개 통과·실패 0개·4개 ignored**, app 전체 타깃 Clippy `-D warnings`를 통과했다. 전체 Windows/peets 검사와 native 파일/PDF 회귀는 통합 검증 기록에서 별도로 확인한다.

재현 명령은 `cargo test -p mathcraft-app edit_commit -- --nocapture`다. ignored `preparation_phase_profile`·`benchmark_draft_commit_and_worker_latency`도 각각 실행했다. 새 SHA 직접 의존으로 현재 Windows test harness는 `mathcraft-33deb4993ecd4ee5.exe`이며 일반 앱 main을 실행하지 않는다.

Ignored 산출물:

- `artifacts/rea/signature-readonly-probe-20261019.{rs,exe,log}`: 읽기 전용 후보 비교, 15회 median과 JSON/hash 차등 검사.
- `artifacts/rea/signature-cache-{tests,clippy,after-profile,after-events,before-events}-20261019.log`: 제품 경로 검사와 단일 probe 표본.
- `artifacts/rea/measure-signature-cache-20261019.py`: 5회 교대 비교 runner.
- `artifacts/rea/signature-cache-alternating-20261019.{json,log}`: 두 실행 파일 SHA·원시 출력·모든 표본·median.

`signature-cache-isolated`는 engine의 다른 병렬 수정이 compile 중이던 시점에 분리 실행을 시도한 흔적이며 링크 단계에서 실행되지 않았다. 검증 근거에는 위 성공한 Cargo 검사와 최종 harness 출력만 사용했다.
