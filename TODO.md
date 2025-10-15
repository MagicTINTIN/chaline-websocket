# TODO

-[ ] Hashmap\<String, Vec\<mpsc::UnboundedSender\<Message\>\>\> to get all connected user for each room (without the need of iterate all the connected user). Think to clean after closing ws, and after some time removing rooms (considered as closed)