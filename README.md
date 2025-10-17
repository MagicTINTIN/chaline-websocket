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
- Broadcast mode with authorized messages with a different reply
```json
{
    "name":"MicaSend",
    "type":"broadcast",
    "prefix":"micasend",
    "map": {
        "new micasend message":"new message notification",
        "ping":"pong"
    }
}
```
- Groups mode with some messages authorized (that will be broadcasted)
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