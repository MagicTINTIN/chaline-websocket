# Chaline's websocket

The configuration file that store the different rooms of the websocket\
configs.json
```json
{
    "name": "Chaline configuration",
    "rooms": [
        "/home/user/Documents/code/rust/chaline-websocket/micasend-ws.json",
        "/home/user/Documents/code/rust/chaline-websocket/nokertu-ws.json"
    ]
}
```

Each room configuration is defined like this
- Broadcast mode
```json
{
    "name":"MicaSend",
    "type":"broadcast",
    "prefix":"micasend"
}
```
- Groups mode
```json
{
    "name":"Clavardons",
    "type":"group",
    "prefix":"clavardons",
    "fetchURL":"clavardons.magictintin.fr/api/ws/group?id="
}
```
- Individual mode
```json
{
    "name":"JIRSend",
    "type":"individual",
    "prefix":"jirsend",
    "fetchURL":"jirsend.magictintin.fr/api/ws/user?id="

}
```