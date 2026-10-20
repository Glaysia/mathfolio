# XY 실패·빈 결과·Pending의 native cache 계약

2026-10-19. Prime12.0.0.1 설치 REA metadata/CIL, 원본 embedded XSD, 설치 worksheet package의 ZIP/XML을 정적으로 조사했다. GUI·스크린샷·원본 engine 함수·runtime·CLR을 실행하지 않았다. 제품 Rust 파일은 수정하지 않았다. [0143](0143-native-result-persistence.md), [0148](0148-native-xy-result-contract.md), [0163](0163-native-empty-and-partial-plot-contract.md)을 잇는다.

## 저장 상태는 Failed/Empty enum이 아니다

result10.xsd의 resultData는 required signed-int result-id, optional calculation-status, optional tag를 갖는다. status에 허용되는 값은 **Synchronized/Pending**뿐이다. Failed와 Empty는 앱 내부 outcome일 수 있지만 native status 문자열로 쓰지 않는다. 자식 순서는 optional result choice(ml:result/ml:symResult/ml:Trace2dResult), optional engineErrors, optional Warnings다. 값 자식이 전혀 없는 row도 허용된다.

ResultSerializer.WriteXml0bb5는 manager의 각 EngineResult를 독립적으로 순회하여 result-id/tag/status를 쓴 뒤 result node, errors, warnings를 쓴다. Pending이라는 이유로 이전 result node를 생략하는 분기는 없다. 0143의 status-copy EngineResult0051도 node/errors/warnings/tag를 복사한 채 status만 바꾸므로 **historical curve를 포함한 Pending row는 원본에서 유효한 상태**다. 실패 trace를 단순히 Pending으로 표시하는 것만으로 이전 curve가 제거되지는 않는다.

## 새 실패 결과는 curve와 오류를 동시에 갖지 않는다

EngineResult constructors0054/0055는 nonempty error collection과 non-IEmptyResultNode 계산값이 동시에 있으면 InvalidOperationException을 던진다. error-copy constructor0053도 새 오류를 받을 때 새 EmptySimpleResultNode를 만들고 error collection을 넣는다. schema가 두 종류 자식을 허용하더라도 이 managed EngineResult 작성 계약은 계산된 curve와 새 실패 오류의 동시 보존을 허용하지 않는다.

ResultSerializer.AddResultToElement0bb6은 **EmptySimpleResultNode이면 아무 value element도 쓰지 않고 반환**한다. 그래서 오류가 있고 result node가 empty인 일반 fresh error는 value 없는 error-only resultData가 된다. 기존 Trace2dResult 아래에 오류만 덧붙여 native 실패를 표현하는 방식은 이 producer와 다르며 reader의 EngineResult constructor도 거절할 수 있다.

TraceModel.SetError128a가 nullable `_engineResultForSerialization` override를 지운 뒤 base.SetError를 호출하는 사실은0163을 따른다. getter12a8은 override가 없으면 current EngineResult를 반환한다. 이 override clear는 native errors를 전혀 저장하지 않는다는 뜻이 아니다. current EngineResult의 error collection은 공통 writer가 직렬화한다.

## 실제 오류 XML writer와 reader

WriteEngineErrors0bb7은 실제 collection의 각 오류를 engineErrors/engineError로 쓴다. WriteEngineError0bba는 다음을 기록한다.

| XML 위치 | 실제 writer 입력 |
| --- | --- |
| engineError/@error-id | ResultSerializer.AddError가 배정한 문서 내 오류 ID |
| @error-node-handle | EngineError.NodeHandle의 UInt32 문자열 |
| @previous-error-id | OriginalError가 있을 때 FindError로 찾은 ID |
| errorCode | ErrorCode가 있는 분기에서 **EngineError.Message** 문자열; XML-invalid 문자는 제거 |
| errorArgument | EngineError.Argument의 UInt32 문자열 |
| resource-string | ResourceString이 null이 아니면 그 문자열 |

특히 XML의 `errorCode` 이름을 보고 Mathcraft의 짧은 fault code를 그대로 넣지 않는다. 현재 writer는 Message를 넣고 ReadEngineError0bc2도 이 element를 **EngineError.Message**로 읽는다. reader는 NodeHandle/Argument/previous-error relationship을 복원하고 ResultManager.FindCalculatable(result-id)를 ErrorSource로 연결한다. 마지막에 EngineErrorMessages.GetStringByKey(ErrorCode)로 ResourceString을 다시 설정하는 경로도 있다. 사람이 읽는 번역 오류 메시지 하나만으로 이 provenance를 재현할 수 있다는 근거는 없다.

XSD engineErrorType은 error-id가 required이고 errorCode, 최소1개의 errorArgument를 요구한다. error-node-tag/error-tag-arg도 schema에 있지만 이 current writer0bba는 그것들을 쓰지 않고 error-node-handle을 쓴다. NodeHandle/Argument writer는 uint인 반면 schema attr는 xs:int인 지점도 있으므로 모든 unsigned 값을 독립 writer가 유효한 native 값으로 인증한다고 일반화하지 않는다. 원본 코드와 schema의 구체 범위를 구분한다.

Warnings는 대문자 element다. WriteWarning0bb9는 nodeId=EngineRedefInfo.Name, oldType enum 문자열, optional oldLocation을 쓴다. 단순 translated warning 문자열을 이 구조로 복원하거나 실패 상태를 redefinition warning으로 대체하지 않는다.

현재 SavedCalculationRow의 error/warning 문자열 또는 per-trace Failed(errorText)만으로 원본 ErrorCode/Message/Argument/NodeHandle/OriginalError/global error-id를 얻을 수 없다. 따라서 **현재 입력 carrier에서 fresh native 오류 payload를 새로 쓰는 것은 미지원**이다. 원본 raw diagnostic row를 그대로 보존할 수 있는 경우는 source/context/trace identity가 증명된 기존 보존 경로이며, 새 계산 실패의 native error 코드가 확인되었다는 뜻이 아니다. PH의 engine error code는 이번에도 실행하거나 추측하지 않았다.

## 빈 row reader와 live dispatch의 중요한 차이

ReadXml0bbe는 값 자식이 없으면 null XML을 XmcdToIResultNodeTransform0227에 넘긴다. 0227은 null 또는 자식 없는 XML에서 CreateEmptyResultNode를 호출한다. 이어 errors/warnings/status/tag와 함께 EngineResult를 만들고 result-id로 manager에 등록한다. 따라서 아래처럼 value 없는 row는 parser가 이전 Trace2dResult를 찾아 대신 복구하는 형태가 아니다.

```xml
<resultData xmlns="http://schemas.mathsoft.com/result10"
            result-id="17" calculation-status="Pending"/>
```

그러나 **이 row가 live owner에 강제 ClearPoints 이벤트를 준다고 결론내리면 안 된다.** EngineResult.IsEmpty0057은 ResultNode가 null 또는 IEmptyResultNode이고 errors가 없으며 warnings도 없으면 true다. ResultManager.DispatchResult0159는 IsEmpty이면 즉시 반환한다. 따라서 diagnostics 없는 empty Pending/Synchronized row를 live owner에 dispatch해도 SetResult(Empty)까지 가지 않는 경로가 있다.

errors가 있는 error-only row는 IsEmpty=false이므로 단일 owner에 status/SetResult/errors가 전달된다. TraceModel.DoSetResult128c는 해당 trace의 points를 clear하고 IEmptyResultNode를 처리하며, SetError 경로도 자기 points를 clear한다. 0163의 trace-local 실패 표시 계약과 연결된다. 여기에서 sibling trace의 정상 resultData를 폐기할 이유는 없다.

**fresh reopen**은 기존 curve object가 없는 새 plot에서 시작한다. old Trace2dResult가 파일 row에서 실제로 제거되어 있으면 그 row로부터 이전 curve points를 복구할 payload가 없다. 이것과 이미 떠 있는 live plot에 clear 이벤트를 강제로 전송하는 것은 서로 다른 계약이다. 이 조사 중 잠정 설명에서 payload 없는 row→SetResult(Empty)→live clear로 연결한 해석은0057 확인 후 철회했으며, 최종 정책은 이 구분을 따른다.

## 앱 export에 적용 가능한 독립 정책

다음 세 작업을 명시적으로 구분하는 것이 현재 증거에 맞는다.

| 작업 | native row 처리 | 의미 |
| --- | --- | --- |
| PreserveHistoricalPending | 기존 payload/errors/warnings를 유지하고 Pending | 이전 표시를 복구할 수 있으나 fresh 계산 성공을 인증하지 않음 |
| WriteFreshFiniteCurve | 인증된 trace 결과만 새 Trace2dResult/Synchronized로 교체 |0148 finite codec 및 source/context/단위/ordinal proof가 필요 |
| ClearCurveToPending | 해당 owned result-id의 **old value/diagnostic 자식을 실제 제거**하고 Pending empty row로 교체 |파일에서 stale curve 재생을 막는 독립 보존 정책; native Failed 오류의 재현이 아님 |

ClearCurveToPending은 upsert(None)가 기존 payload를 유지하는 동작과 다르다. 이전 곡선이 실제로 더 이상 유효하지 않은 fresh Failed/새 Empty 작성 상태를 저장하면서 native diagnostic을 표현할 수 없을 때 사용하는 bounded fallback이다. 앱 자신의 saved outcome/errorText는 별도 .mathcraft carrier로 보존할 수 있다. MCDX에 가짜 errorCode,0값 또는 성공한 빈 curve payload를 만들지 않는다.

root export helper가 허용된 owned row를 바꾸기 전에 resultRef의 유일성, source trace identity, result part/root, duplicate ID 및 unknown metadata 정책을 preflight해야 한다. 이 문서는 unknown 원본 attrs/children을 무조건 삭제하라는 일반 권한을 부여하지 않는다. 새 canonical row에 어떤 tag를 유지할지도 별도 의미 proof가 필요하다. 지원하지 않는 원본 diagnostic이 그대로여야 하는 보존 경로와 새 실패 clear fallback을 분리한다.

PH 또는 앱 Empty를 원본 Synchronized empty 계산 결과로 인증할 수는 없다. native PH 엔진 결과가 무엇인지 미확정이므로 ClearCurveToPending과 새 empty Synchronized row를 혼동하지 않는다. definition의 실제 synchronized empty row는0143에서 관찰됐지만 plot PH의 성공 여부를 증명하지 않는다.

## 원본 package 관찰과 파생 schema fixture

설치 폴더의 원본 .mcdx/.mctx77개/result part77개를 ZIP/XML로 확인했다. engineError0개, Trace2dResult0개였고 scan error0개다. 따라서 이 설치 corpus에서 오류 plot cache의 original serialized example은 얻지 못했다. 사용자가 새로 계산한 앱 출력이나 derived fixture를 원본 error cache라고 부르지 않는다. 행/part SHA와 count는 ignored `native-xy-error-cache-package-observations-20261019.json`에 있다.

독립적으로 조립한 `native-xy-error-cache-empty-derived-20261019.xml`은 diagnostics 없는 Pending/Synchronized empty row 두 개만 갖는다. 수정하지 않은 result10/Math50 XSD와 기존 local XML namespace supplement로 schema 검증을 통과했으며 status=Failed 반례는 거절됐다. 이는 empty row의 schema 허용만 확인하며 PH 계산 성공/원본 runtime acceptance를 증명하지 않는다. 검증 JSON과 재실행 Python을 같은 prefix로 보존했다. 오류 code를 invent한 파생 native error fixture는 만들지 않았다.

## 근거 identity와 미확정

새 Controls serializer REA=`ev_1bc1d4309b220a4d869d89bb5311e1aed1565fbc3d30ad69211686bf25d0c79b`, Calculation Core=`ev_ae359b7e6027564041b11bc39289a6613558779a47be72c5f3747d804347b391`.

Controls.Core SHA=`654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977`, Calculation.Core SHA=`cf42d8116d697a560f84467c8c32eb8dc1a15172205772eaf4c95a1125eb01bf`. 선택 token/IL SHA와 schema/package 관찰 hash는 `native-xy-error-cache-method-ledger-20261019.json`에 기록했다. 기존0143의 raw writer body도 현재 설치 bytes로 재검증했다. 원시 CIL은`feedback/native-xy-error-cache-*-cil-20261019.txt`。

미확정은 PH의 실제 엔진 결과/오류 코드, 모든 native plot failure의 정확한 error message와 node handle, fresh Failure와 Empty에 대한 원본 UI auto-save/commit 타이밍이다. 이번 범위의 완료는 stale curve가 남는 export fallback을 구분할 근거이며 이 미확정을 제품 호환 완료로 계산하지 않는다.
