# Compact Context Rollup Inputs (CTX)

## 버전

`CTX1`은 아직 상위 rollup에 포함되지 않은 파생 요약을 모델에 전달하는 줄 기반 형식이다. UTF-8과 LF를 사용한다.

```text
!CTX1|scope=session|through=42|count=2
C|38|K001|summary: 첫 세션\nimportant: 일정 합의
C|42|K001|summary: 둘째 세션\nopen: 장소 미정
```

## 레코드

- 헤더 `scope`는 입력 요약의 scope다. `session` 입력은 `thread` rollup으로, `thread` 입력은 `global` rollup으로 합친다.
- `through`는 출력에 포함된 가장 큰 context ID다. 생성한 rollup을 저장할 때 `--through-context-id`로 그대로 전달한다.
- `count`는 출력된 `C` 레코드 수다.
- `C`는 `context_id|scope_key|summary` 순서다. 필드의 `\\`, `|`, LF, CR은 CCT와 같은 방식으로 이스케이프한다.

`context inputs thread`는 해당 별칭에서 아직 rollup되지 않은 모든 세션 요약을 출력한다. `context inputs global`은 각 별칭의 누적 thread rollup 중 최신 미반영 버전만 출력해 중간 버전의 토큰을 생략한다.

`context put thread|global --through-context-id <id>`는 대상 scope의 아직 미반영된 입력 중 ID가 `through` 이하인 행을 새 rollup에 원자적으로 연결한다. 출력 이후 생성된 더 큰 ID는 연결하지 않으므로 다음 실행에 남는다. watermark가 낡았거나 다른 scope의 ID면 저장하지 않고 다시 `context inputs`를 요구한다.

## 관련 문서

**메시지 입력** — [CCT](../cct/CCT.md)

**설계** — [DESIGN-PIPE](../../docs/design/pipeline/DESIGN.md)
