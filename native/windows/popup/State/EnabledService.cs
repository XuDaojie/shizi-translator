namespace Shizi.Popup.State;

/// <summary>启用服务元数据，对齐 Vue <c>EnabledServicePayload</c>。</summary>
public sealed class EnabledService
{
    public EnabledService(
        string serviceInstanceId,
        string serviceName = "",
        string serviceType = "",
        string protocol = "",
        string modelName = "")
    {
        ServiceInstanceId = serviceInstanceId;
        ServiceName = serviceName;
        ServiceType = serviceType;
        Protocol = protocol;
        ModelName = modelName;
    }

    public string ServiceInstanceId { get; }
    public string ServiceName { get; }
    public string ServiceType { get; }
    public string Protocol { get; }
    public string ModelName { get; }
}
