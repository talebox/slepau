The Samn project here has everything specific to interact with the radio modules.
This will be the base of operations, the HQ, commander of all nodes. An HQ can also optionally act as a relay, where another HQ project running somewhere else can also manage this HQ and the network. For that we would need to sync states and logs. Perhaps this project could be stateless, that means nodes in the network are shown only once a message has been received from them, that would simplify a lot and make it super rubust, in fact that's what we'll do. Nodes will only check in with HQ periodically, then switch to receive and listen for a small period of time and HQ will have commands queued up that will be sent to the node one by one, Node receives command, does stuff, switches to TX and sends response, repeat... when all commands are done Node won't receive anything and after a period of time it goes to sleep until next sensor reporting interval where it all begins again.

This allow for power saving between reporting intervals and a completely headless Node.

This will expose an HTTP api to send to send/receive messages out to the different modules.

Connections happen from HQ to each Node directly, at least for now.