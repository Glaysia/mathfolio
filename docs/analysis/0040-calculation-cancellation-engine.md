# 계산 취소와 재개를 위한 독립 엔진 구현

2026-10-16. 원본 계약은 [0039 계산 큐와 취소](0039-calculation-queue-and-cancellation.md), UI·백그라운드 통합은 [0041](0041-background-calculation-ui.md)을 기준으로 한다. GUI·스크린샷·Mathcad 런타임을 사용하지 않았다.

## 계산 제어 API

`mathcraft-compat`의 `CalculationControl`은 작업별 `Arc<AtomicBool>` 취소 요청이다. UI 스레드는 복제한 핸들의 `cancel()`을 호출하고, 계산 스레드는 비용이 큰 반복의 경계에서 이를 확인한다. 원본 네이티브 스레드 중단 코드를 복제하거나 운영체제의 강제 스레드 종료를 호출하지 않는다.

| API | 동작 |
| --- | --- |
| `CalculationControl::default()` | 취소되지 않은 새 작업 제어 객체 |
| `cancel()` / `is_cancelled()` | 다른 스레드에서도 사용 가능한 중단 요청·조회 |
| `run(operation)` | 현재 계산 스레드에 제어 객체를 적용하며 중첩 호출·반환·panic unwind 시 이전 상태 복원 |
| `WorksheetCalculator::calculate_with_control(request, &control)` | 해당 제어 객체를 적용한 계산 |
| `CalculationUpdate.cancelled` | 일반 수식 오류와 구분되는 중단 결과 |
| `Fault::is_cancelled()` | 임시 수식·도표 질의의 `CANCELLED` 제어 신호 구분 |

기존 `calculate`·`evaluate_worksheet` 계열은 제어 객체 없이 같은 계산 동작을 유지한다. 작업마다 새 제어 객체를 사용한다. 함수 정의가 캡처한 Scope에는 작업 제어 객체를 저장하지 않으므로 이전 작업 취소가 다음 작업의 기존 함수를 영구적으로 취소하지 않는다.

## 실행 도중의 일관성

1. 완료된 영역만 결과와 바인딩 변경분을 확정한다. 취소된 현재 영역은 이전 표시를 유지하며 미계산 상태로 남긴다. `CANCELLED`를 새 오류 결과로 게시하지 않는다.
2. 정의나 solve-block 출력처럼 워크시트 바인딩을 바꾸는 영역은 해당 출력 이름들의 이전 값만 보관한다. 취소 시 이 값들을 복원한다. 영역마다 전체 Scope를 다시 복사하는 방식은 사용하지 않는다. 일반적인 계산 실패의 기존 `Binding::Failed` 전파는 유지하되 취소는 실패 바인딩을 만들지 않는다.
3. 전역 정의의 선행 계산은 완료된 전역 바인딩과 표시 후보를 보존한다. 취소된 전역 정의는 확정하지 않으며 부분 전역 Scope를 도표 질의에 제공하지 않는다. 완료된 전역 결과도 원래의 계산 범위에 도달하기 전에는 워크시트 행에 새 결과로 게시하지 않는다.
4. 새 작업에서 `ThroughRegion`을 요청하면 완료된 바인딩 변경분을 재사용하고 미계산 영역부터 이어간다. `Worksheet`는 기존 완료 상태와 전역 선행 계산을 모두 무효화한 후 전체를 강제 계산한다.
5. 원본을 갱신하는 동안 중단되면 파싱 완료 후 취소를 확인해 새 입력을 확정하지 않는다. 다음 작업에서 같은 새 원본을 다시 준비할 수 있다. 파싱만 수행하는 작업은 수식을 실행하지 않는다.

Stop에 따른 자동→수동 전환, 마지막 화면 범위, 작업 세대 및 새 문서 전환은 UI·worker 계층의 책임이다. 엔진의 취소 신호 자체가 워크시트를 영구 비활성화하지 않는다.

## 취소 확인 위치와 한계

수식 재귀 평가와 함수 이름 검증, 프로그램 명령·반복, 수치 연산의 표본 예산 확인, 행렬 요소 검사와 분해·고유값·거듭제곱 반복, solve 잔차·선 탐색, 기호 다항식 연산에 협력적 확인을 넣었다. 따라서 영역 사이에서만 취소를 확인하며 긴 프로그램 전체가 끝나기를 기다리는 구조는 아니다.

이는 독립 구현의 협력적 취소 정책이다. XML 파서·표준 라이브러리 정렬·메모리 할당·개별 수학 intrinsic 등의 단일 호출을 중간에서 강제 종료하지 않는다. 작업 준비와 결과 스냅샷 복사도 모든 명령 단위에서 중단 가능한 것은 아니다. 기존 입력 크기·행렬·표본·명령 예산이 적용되지만, 일정한 밀리초 안에 모든 계산이 멈춘다는 보장은 측정하거나 주장하지 않는다. 원본 엔진의 모든 정밀 의존성·스레드 스케줄링을 복원한 것도 아니다.

## headless 회귀

`src/cancellation_tests.rs`의 제한된 테스트 제어 객체는 확인 횟수를 기준으로 중단한다. sleep이나 화면 상태에 의존하지 않고 한 영역의 내부 반복 중간을 재현하기 위한 테스트 전용 장치이며 제품 API에는 노출하지 않는다.

| 테스트 | 검증 |
| --- | --- |
| `cancellation_control_is_thread_safe_scoped_and_not_a_worksheet_fault` | 다른 스레드의 취소, 중첩 제어 범위 복원, 사전 취소, 오류 없이 미계산 상태로 재개 |
| `cancellation_inside_program_preserves_previous_payload_and_resumes_only_pending_regions` | 긴 프로그램 내부 중단, 이전 표시 보존, 완료된 앞 영역 재실행 없음 |
| `cancellation_of_global_prepass_keeps_completed_bindings_without_publishing_partial_rows` | 부분 전역 상태의 안전한 재사용·비게시, 이후 전체 계산 강제 실행 |
| `cancelling_an_old_job_does_not_cancel_functions_captured_by_a_new_job` | 작업 사이에 유지한 함수 바인딩의 취소 객체 오염 방지 |
| `cancelled_definitions_and_solver_outputs_do_not_poison_existing_bindings` | 실제 정의·solve 경로에서 이전 바인딩 보존 |
| `cancellation_checks_reach_numeric_and_array_work_inside_one_expression` | 행렬 역행렬과 polyroots 내부의 중단 신호 |

운영체제별 최종 전체 검증 결과는 [CONFORMANCE](../CONFORMANCE.md)를 따른다.
