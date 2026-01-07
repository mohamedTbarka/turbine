from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class TaskState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    TASK_STATE_UNSPECIFIED: _ClassVar[TaskState]
    TASK_STATE_PENDING: _ClassVar[TaskState]
    TASK_STATE_RECEIVED: _ClassVar[TaskState]
    TASK_STATE_RUNNING: _ClassVar[TaskState]
    TASK_STATE_SUCCESS: _ClassVar[TaskState]
    TASK_STATE_FAILURE: _ClassVar[TaskState]
    TASK_STATE_RETRY: _ClassVar[TaskState]
    TASK_STATE_REVOKED: _ClassVar[TaskState]
TASK_STATE_UNSPECIFIED: TaskState
TASK_STATE_PENDING: TaskState
TASK_STATE_RECEIVED: TaskState
TASK_STATE_RUNNING: TaskState
TASK_STATE_SUCCESS: TaskState
TASK_STATE_FAILURE: TaskState
TASK_STATE_RETRY: TaskState
TASK_STATE_REVOKED: TaskState

class TaskOptions(_message.Message):
    __slots__ = ("queue", "priority", "max_retries", "retry_delay", "timeout", "soft_timeout", "eta", "countdown", "expires", "store_result", "result_ttl", "idempotency_key", "headers")
    class HeadersEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    QUEUE_FIELD_NUMBER: _ClassVar[int]
    PRIORITY_FIELD_NUMBER: _ClassVar[int]
    MAX_RETRIES_FIELD_NUMBER: _ClassVar[int]
    RETRY_DELAY_FIELD_NUMBER: _ClassVar[int]
    TIMEOUT_FIELD_NUMBER: _ClassVar[int]
    SOFT_TIMEOUT_FIELD_NUMBER: _ClassVar[int]
    ETA_FIELD_NUMBER: _ClassVar[int]
    COUNTDOWN_FIELD_NUMBER: _ClassVar[int]
    EXPIRES_FIELD_NUMBER: _ClassVar[int]
    STORE_RESULT_FIELD_NUMBER: _ClassVar[int]
    RESULT_TTL_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    HEADERS_FIELD_NUMBER: _ClassVar[int]
    queue: str
    priority: int
    max_retries: int
    retry_delay: int
    timeout: int
    soft_timeout: int
    eta: int
    countdown: int
    expires: int
    store_result: bool
    result_ttl: int
    idempotency_key: str
    headers: _containers.ScalarMap[str, str]
    def __init__(self, queue: _Optional[str] = ..., priority: _Optional[int] = ..., max_retries: _Optional[int] = ..., retry_delay: _Optional[int] = ..., timeout: _Optional[int] = ..., soft_timeout: _Optional[int] = ..., eta: _Optional[int] = ..., countdown: _Optional[int] = ..., expires: _Optional[int] = ..., store_result: bool = ..., result_ttl: _Optional[int] = ..., idempotency_key: _Optional[str] = ..., headers: _Optional[_Mapping[str, str]] = ...) -> None: ...

class Task(_message.Message):
    __slots__ = ("id", "name", "args", "kwargs", "options")
    class KwargsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: bytes
        def __init__(self, key: _Optional[str] = ..., value: _Optional[bytes] = ...) -> None: ...
    ID_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    ARGS_FIELD_NUMBER: _ClassVar[int]
    KWARGS_FIELD_NUMBER: _ClassVar[int]
    OPTIONS_FIELD_NUMBER: _ClassVar[int]
    id: str
    name: str
    args: _containers.RepeatedScalarFieldContainer[bytes]
    kwargs: _containers.ScalarMap[str, bytes]
    options: TaskOptions
    def __init__(self, id: _Optional[str] = ..., name: _Optional[str] = ..., args: _Optional[_Iterable[bytes]] = ..., kwargs: _Optional[_Mapping[str, bytes]] = ..., options: _Optional[_Union[TaskOptions, _Mapping]] = ...) -> None: ...

class TaskMeta(_message.Message):
    __slots__ = ("state", "retries", "created_at", "started_at", "finished_at", "worker_id", "error", "traceback")
    STATE_FIELD_NUMBER: _ClassVar[int]
    RETRIES_FIELD_NUMBER: _ClassVar[int]
    CREATED_AT_FIELD_NUMBER: _ClassVar[int]
    STARTED_AT_FIELD_NUMBER: _ClassVar[int]
    FINISHED_AT_FIELD_NUMBER: _ClassVar[int]
    WORKER_ID_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    TRACEBACK_FIELD_NUMBER: _ClassVar[int]
    state: TaskState
    retries: int
    created_at: int
    started_at: int
    finished_at: int
    worker_id: str
    error: str
    traceback: str
    def __init__(self, state: _Optional[_Union[TaskState, str]] = ..., retries: _Optional[int] = ..., created_at: _Optional[int] = ..., started_at: _Optional[int] = ..., finished_at: _Optional[int] = ..., worker_id: _Optional[str] = ..., error: _Optional[str] = ..., traceback: _Optional[str] = ...) -> None: ...

class TaskResult(_message.Message):
    __slots__ = ("task_id", "state", "result", "error", "traceback", "created_at")
    TASK_ID_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    RESULT_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    TRACEBACK_FIELD_NUMBER: _ClassVar[int]
    CREATED_AT_FIELD_NUMBER: _ClassVar[int]
    task_id: str
    state: TaskState
    result: bytes
    error: str
    traceback: str
    created_at: int
    def __init__(self, task_id: _Optional[str] = ..., state: _Optional[_Union[TaskState, str]] = ..., result: _Optional[bytes] = ..., error: _Optional[str] = ..., traceback: _Optional[str] = ..., created_at: _Optional[int] = ...) -> None: ...

class SubmitTaskRequest(_message.Message):
    __slots__ = ("name", "args", "kwargs", "options", "task_id", "correlation_id")
    class KwargsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: bytes
        def __init__(self, key: _Optional[str] = ..., value: _Optional[bytes] = ...) -> None: ...
    NAME_FIELD_NUMBER: _ClassVar[int]
    ARGS_FIELD_NUMBER: _ClassVar[int]
    KWARGS_FIELD_NUMBER: _ClassVar[int]
    OPTIONS_FIELD_NUMBER: _ClassVar[int]
    TASK_ID_FIELD_NUMBER: _ClassVar[int]
    CORRELATION_ID_FIELD_NUMBER: _ClassVar[int]
    name: str
    args: _containers.RepeatedScalarFieldContainer[bytes]
    kwargs: _containers.ScalarMap[str, bytes]
    options: TaskOptions
    task_id: str
    correlation_id: str
    def __init__(self, name: _Optional[str] = ..., args: _Optional[_Iterable[bytes]] = ..., kwargs: _Optional[_Mapping[str, bytes]] = ..., options: _Optional[_Union[TaskOptions, _Mapping]] = ..., task_id: _Optional[str] = ..., correlation_id: _Optional[str] = ...) -> None: ...

class SubmitTaskResponse(_message.Message):
    __slots__ = ("task_id", "state")
    TASK_ID_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    task_id: str
    state: TaskState
    def __init__(self, task_id: _Optional[str] = ..., state: _Optional[_Union[TaskState, str]] = ...) -> None: ...

class SubmitBatchRequest(_message.Message):
    __slots__ = ("tasks",)
    TASKS_FIELD_NUMBER: _ClassVar[int]
    tasks: _containers.RepeatedCompositeFieldContainer[SubmitTaskRequest]
    def __init__(self, tasks: _Optional[_Iterable[_Union[SubmitTaskRequest, _Mapping]]] = ...) -> None: ...

class SubmitBatchResponse(_message.Message):
    __slots__ = ("task_ids", "count")
    TASK_IDS_FIELD_NUMBER: _ClassVar[int]
    COUNT_FIELD_NUMBER: _ClassVar[int]
    task_ids: _containers.RepeatedScalarFieldContainer[str]
    count: int
    def __init__(self, task_ids: _Optional[_Iterable[str]] = ..., count: _Optional[int] = ...) -> None: ...

class GetTaskStatusRequest(_message.Message):
    __slots__ = ("task_id",)
    TASK_ID_FIELD_NUMBER: _ClassVar[int]
    task_id: str
    def __init__(self, task_id: _Optional[str] = ...) -> None: ...

class GetTaskStatusResponse(_message.Message):
    __slots__ = ("task_id", "meta")
    TASK_ID_FIELD_NUMBER: _ClassVar[int]
    META_FIELD_NUMBER: _ClassVar[int]
    task_id: str
    meta: TaskMeta
    def __init__(self, task_id: _Optional[str] = ..., meta: _Optional[_Union[TaskMeta, _Mapping]] = ...) -> None: ...

class GetTaskResultRequest(_message.Message):
    __slots__ = ("task_id",)
    TASK_ID_FIELD_NUMBER: _ClassVar[int]
    task_id: str
    def __init__(self, task_id: _Optional[str] = ...) -> None: ...

class GetTaskResultResponse(_message.Message):
    __slots__ = ("result", "ready")
    RESULT_FIELD_NUMBER: _ClassVar[int]
    READY_FIELD_NUMBER: _ClassVar[int]
    result: TaskResult
    ready: bool
    def __init__(self, result: _Optional[_Union[TaskResult, _Mapping]] = ..., ready: bool = ...) -> None: ...

class WaitForResultRequest(_message.Message):
    __slots__ = ("task_id", "timeout")
    TASK_ID_FIELD_NUMBER: _ClassVar[int]
    TIMEOUT_FIELD_NUMBER: _ClassVar[int]
    task_id: str
    timeout: int
    def __init__(self, task_id: _Optional[str] = ..., timeout: _Optional[int] = ...) -> None: ...

class WaitForResultResponse(_message.Message):
    __slots__ = ("result", "success")
    RESULT_FIELD_NUMBER: _ClassVar[int]
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    result: TaskResult
    success: bool
    def __init__(self, result: _Optional[_Union[TaskResult, _Mapping]] = ..., success: bool = ...) -> None: ...

class RevokeTaskRequest(_message.Message):
    __slots__ = ("task_id", "terminate")
    TASK_ID_FIELD_NUMBER: _ClassVar[int]
    TERMINATE_FIELD_NUMBER: _ClassVar[int]
    task_id: str
    terminate: bool
    def __init__(self, task_id: _Optional[str] = ..., terminate: bool = ...) -> None: ...

class RevokeTaskResponse(_message.Message):
    __slots__ = ("success", "previous_state")
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    PREVIOUS_STATE_FIELD_NUMBER: _ClassVar[int]
    success: bool
    previous_state: TaskState
    def __init__(self, success: bool = ..., previous_state: _Optional[_Union[TaskState, str]] = ...) -> None: ...

class RetryTaskRequest(_message.Message):
    __slots__ = ("task_id",)
    TASK_ID_FIELD_NUMBER: _ClassVar[int]
    task_id: str
    def __init__(self, task_id: _Optional[str] = ...) -> None: ...

class RetryTaskResponse(_message.Message):
    __slots__ = ("new_task_id", "success")
    NEW_TASK_ID_FIELD_NUMBER: _ClassVar[int]
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    new_task_id: str
    success: bool
    def __init__(self, new_task_id: _Optional[str] = ..., success: bool = ...) -> None: ...

class GetQueueInfoRequest(_message.Message):
    __slots__ = ("queue",)
    QUEUE_FIELD_NUMBER: _ClassVar[int]
    queue: str
    def __init__(self, queue: _Optional[str] = ...) -> None: ...

class QueueInfo(_message.Message):
    __slots__ = ("name", "pending", "processing", "consumers", "throughput")
    NAME_FIELD_NUMBER: _ClassVar[int]
    PENDING_FIELD_NUMBER: _ClassVar[int]
    PROCESSING_FIELD_NUMBER: _ClassVar[int]
    CONSUMERS_FIELD_NUMBER: _ClassVar[int]
    THROUGHPUT_FIELD_NUMBER: _ClassVar[int]
    name: str
    pending: int
    processing: int
    consumers: int
    throughput: float
    def __init__(self, name: _Optional[str] = ..., pending: _Optional[int] = ..., processing: _Optional[int] = ..., consumers: _Optional[int] = ..., throughput: _Optional[float] = ...) -> None: ...

class GetQueueInfoResponse(_message.Message):
    __slots__ = ("queues",)
    QUEUES_FIELD_NUMBER: _ClassVar[int]
    queues: _containers.RepeatedCompositeFieldContainer[QueueInfo]
    def __init__(self, queues: _Optional[_Iterable[_Union[QueueInfo, _Mapping]]] = ...) -> None: ...

class PurgeQueueRequest(_message.Message):
    __slots__ = ("queue",)
    QUEUE_FIELD_NUMBER: _ClassVar[int]
    queue: str
    def __init__(self, queue: _Optional[str] = ...) -> None: ...

class PurgeQueueResponse(_message.Message):
    __slots__ = ("purged",)
    PURGED_FIELD_NUMBER: _ClassVar[int]
    purged: int
    def __init__(self, purged: _Optional[int] = ...) -> None: ...

class SubscribeEventsRequest(_message.Message):
    __slots__ = ("task_ids", "event_types")
    TASK_IDS_FIELD_NUMBER: _ClassVar[int]
    EVENT_TYPES_FIELD_NUMBER: _ClassVar[int]
    task_ids: _containers.RepeatedScalarFieldContainer[str]
    event_types: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, task_ids: _Optional[_Iterable[str]] = ..., event_types: _Optional[_Iterable[str]] = ...) -> None: ...

class TaskEvent(_message.Message):
    __slots__ = ("event_type", "task_id", "task_name", "state", "timestamp", "data")
    EVENT_TYPE_FIELD_NUMBER: _ClassVar[int]
    TASK_ID_FIELD_NUMBER: _ClassVar[int]
    TASK_NAME_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    TIMESTAMP_FIELD_NUMBER: _ClassVar[int]
    DATA_FIELD_NUMBER: _ClassVar[int]
    event_type: str
    task_id: str
    task_name: str
    state: TaskState
    timestamp: int
    data: bytes
    def __init__(self, event_type: _Optional[str] = ..., task_id: _Optional[str] = ..., task_name: _Optional[str] = ..., state: _Optional[_Union[TaskState, str]] = ..., timestamp: _Optional[int] = ..., data: _Optional[bytes] = ...) -> None: ...

class HealthCheckRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class HealthCheckResponse(_message.Message):
    __slots__ = ("status", "version", "uptime", "broker_status", "backend_status")
    STATUS_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    UPTIME_FIELD_NUMBER: _ClassVar[int]
    BROKER_STATUS_FIELD_NUMBER: _ClassVar[int]
    BACKEND_STATUS_FIELD_NUMBER: _ClassVar[int]
    status: str
    version: str
    uptime: int
    broker_status: str
    backend_status: str
    def __init__(self, status: _Optional[str] = ..., version: _Optional[str] = ..., uptime: _Optional[int] = ..., broker_status: _Optional[str] = ..., backend_status: _Optional[str] = ...) -> None: ...

class SubmitChainRequest(_message.Message):
    __slots__ = ("tasks", "options")
    TASKS_FIELD_NUMBER: _ClassVar[int]
    OPTIONS_FIELD_NUMBER: _ClassVar[int]
    tasks: _containers.RepeatedCompositeFieldContainer[SubmitTaskRequest]
    options: ChainOptions
    def __init__(self, tasks: _Optional[_Iterable[_Union[SubmitTaskRequest, _Mapping]]] = ..., options: _Optional[_Union[ChainOptions, _Mapping]] = ...) -> None: ...

class ChainOptions(_message.Message):
    __slots__ = ("stop_on_failure", "pass_results")
    STOP_ON_FAILURE_FIELD_NUMBER: _ClassVar[int]
    PASS_RESULTS_FIELD_NUMBER: _ClassVar[int]
    stop_on_failure: bool
    pass_results: bool
    def __init__(self, stop_on_failure: bool = ..., pass_results: bool = ...) -> None: ...

class SubmitGroupRequest(_message.Message):
    __slots__ = ("tasks", "options")
    TASKS_FIELD_NUMBER: _ClassVar[int]
    OPTIONS_FIELD_NUMBER: _ClassVar[int]
    tasks: _containers.RepeatedCompositeFieldContainer[SubmitTaskRequest]
    options: GroupOptions
    def __init__(self, tasks: _Optional[_Iterable[_Union[SubmitTaskRequest, _Mapping]]] = ..., options: _Optional[_Union[GroupOptions, _Mapping]] = ...) -> None: ...

class GroupOptions(_message.Message):
    __slots__ = ("max_concurrency", "continue_on_failure")
    MAX_CONCURRENCY_FIELD_NUMBER: _ClassVar[int]
    CONTINUE_ON_FAILURE_FIELD_NUMBER: _ClassVar[int]
    max_concurrency: int
    continue_on_failure: bool
    def __init__(self, max_concurrency: _Optional[int] = ..., continue_on_failure: bool = ...) -> None: ...

class SubmitChordRequest(_message.Message):
    __slots__ = ("group", "callback", "options")
    GROUP_FIELD_NUMBER: _ClassVar[int]
    CALLBACK_FIELD_NUMBER: _ClassVar[int]
    OPTIONS_FIELD_NUMBER: _ClassVar[int]
    group: SubmitGroupRequest
    callback: SubmitTaskRequest
    options: ChordOptions
    def __init__(self, group: _Optional[_Union[SubmitGroupRequest, _Mapping]] = ..., callback: _Optional[_Union[SubmitTaskRequest, _Mapping]] = ..., options: _Optional[_Union[ChordOptions, _Mapping]] = ...) -> None: ...

class ChordOptions(_message.Message):
    __slots__ = ("pass_results", "execute_on_partial_failure")
    PASS_RESULTS_FIELD_NUMBER: _ClassVar[int]
    EXECUTE_ON_PARTIAL_FAILURE_FIELD_NUMBER: _ClassVar[int]
    pass_results: bool
    execute_on_partial_failure: bool
    def __init__(self, pass_results: bool = ..., execute_on_partial_failure: bool = ...) -> None: ...

class SubmitWorkflowResponse(_message.Message):
    __slots__ = ("workflow_id", "task_ids")
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    TASK_IDS_FIELD_NUMBER: _ClassVar[int]
    workflow_id: str
    task_ids: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, workflow_id: _Optional[str] = ..., task_ids: _Optional[_Iterable[str]] = ...) -> None: ...

class GetWorkflowStatusRequest(_message.Message):
    __slots__ = ("workflow_id",)
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    workflow_id: str
    def __init__(self, workflow_id: _Optional[str] = ...) -> None: ...

class GetWorkflowStatusResponse(_message.Message):
    __slots__ = ("workflow_id", "state", "task_statuses", "completed", "total")
    WORKFLOW_ID_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    TASK_STATUSES_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_FIELD_NUMBER: _ClassVar[int]
    TOTAL_FIELD_NUMBER: _ClassVar[int]
    workflow_id: str
    state: TaskState
    task_statuses: _containers.RepeatedCompositeFieldContainer[TaskMeta]
    completed: int
    total: int
    def __init__(self, workflow_id: _Optional[str] = ..., state: _Optional[_Union[TaskState, str]] = ..., task_statuses: _Optional[_Iterable[_Union[TaskMeta, _Mapping]]] = ..., completed: _Optional[int] = ..., total: _Optional[int] = ...) -> None: ...
