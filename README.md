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
- Broadcast mode with only one message authorized
```json
{
    "name":"MicaSend",
    "type":"broadcast",
    "prefix":"micasend",
    "authorized": ["ping"]
}
```
- Groups mode with some messages authorized
```json
{
    "name":"Clavardons",
    "type":"group",
    "prefix":"clavardons",
    "fetchURL":"clavardons.magictintin.fr/api/ws/group?id=",
    "authorized": ["join", "leave"]
}
```
- **NOT IMPLEMENTED YET** *~~Individual mode~~* with all messages allowed ("authorized": [])
```json
{
    "name":"JIRSend",
    "type":"individual",
    "prefix":"jirsend",
    "fetchURL":"jirsend.magictintin.fr/api/ws/user?id="
}
```