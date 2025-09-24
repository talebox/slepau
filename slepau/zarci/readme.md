# Zarci - A store backend

Really the hardest part about a store's technical aspect is inventory management and payments. Marketing is a whole other beast.

I'm using what I learned about building the [Sandal store project on Firebase](https://sandals-f66c3.web.app/).

Let's focus on an MVP that works and we can start marketing with while the boards arrive and/or we figure out Amazon selling.


## Product

A product has name, description, pictures, price, and a list of choices.

## Inventory

inventory management (best done through events + accumulator to extract current state)

inventory management deals with product_hash (id + choice_ids) numbers

a product hash let's say is (product_id u16, choice_ids u16 (enough to store 4x 16 choices, bc we don't need more))

Product ids have to be physically visible on product itself while product hash is a derived property of the products being identical.

Ok so we have a product hash let's talk about inventory event types:
- Shipment (an order placed at manufacturer, with an expected stocking date)
- Stocking (added to inventory, including product ids)
- Lost (missing or damaged product, subtracts from available inventory)
- Order (a customer ordered a product, payment has been successful, whether online/physical, if physical then `shipped` status is meaningless)
- Cancel (a customer canceled an order)
- Shipped (an order has been packed, tracking is available)

Ok when thinking about returns what if customer purchased 2 products but only 1 of them is defective, so they return that one item and that item is then restocked. And this is why amazon manages products as items, they probably group items into various easier to track orders, but they still manage each item individually unerneath. This means I might need to change the way I track inventory, say I can't just track products based on their product hashes, but also based on their specific unique product ids so that later I can pull up that specifc product's history.

I think the 4 inventory events (Shipment, Stocking, Order, Shipped) cover the basic product lifecyle inside a store of items will be availble (Shipment), they are available (Stocking) to to item sold (Order), to the online specific packed and shipped (Shipped). The first event of (Shipment) is even extra just to give users some heads up about when it'll be available thus not really strictly necessary, just nice to have for transparency.

I've added Lost (to be able to remove available or inventory items) and Cancel so a customer can cancel an order that hasn't been shipped out yet.



Regarding customers purchasing, should there be a hold period where products are removed from the inventory temporarily while customers check out?
It would be complex to setup so I'll just go with removed from invetory once checkout has happened and order is complete.

---

I think it's better to build the website and then define backend sources that will feed onto that website.


I think for the Web UI it'd be better to make a text parser to simplify a lot of these processes and allow for easy module testing of fuctionality, something like this V. To make the store easy to setup in the beginning and so that store state is easy to visualize later on.

Awesome I'll use toml, seems appropiate for the task at hand, just complex enough with easy human parsing as well. 

```toml

[store]
# Translates to "store owned by Logji."
name = "Zarci po la .Logjis."

[product.locus_molus]
name = "2.4GHz Module (Nrf24l01+)"
description =  "A thoroughly designed nrf24 module that's orders of magnitude better than the other cheap chinese boards you'll find elsewhere.

This means:
- A genuine NRF24L01+ chip.
- A proper balun circuit (the one recommended by Nordic). 
- A larger pcb antenna that isn't as sensitive to environmental detuning.
- And finally, properly placed decoupling capacitors (2x10uF & 3x100nF).

Make sure front and back of antenna isn't obstructed by walls/fingers. This design is more <b>resistant</b> to detuning because of a larger resonnant structure, <b>resistant</b>, not impermeable.

Range depends highly on antenna tunning and noise in the 2.4GHz range. The maximum I was able to archieve in a relatively noisy environment (apartment building with lots of wifi) with line of sight was about 100 meters. Indoors it's about 15-20 meters. 

Range tests were done on the highest output power with multiple retransmissions.


<b>Genuine</b> NRF24L01+ chip
<b>Proper balun</b> circuit (the one recommended by Nordic)
<b>Larger PCB antenna</b> that isn't as sensitive to environmental detuning
<b>Properly placed decoupling capacitors<b/> (2x10uF & 3x100nF)"
price = 10
# pictures: ["losab_barub_sanib_locus"] this is filled out by the UI when pictures are attached to a product.

[product.malab_taruv]
name = "915MHz Module (CC1101)"
description = "A 915MHz module for products that need the higher reliability and longer range of a 915MHz connection. The only cons are reduced bits per second and lower level programming burden on the MCU that the chip requires. The small cons are well worth the big leap in reliability though, in my experience."
price = 20

```



## Payment (Stripe)

What a minimal “own UI” card flow looks like:

* **Server (create PaymentIntent)**

  * POST `/payment_intents` with `amount`, `currency`, `automatic_payment_methods[enabled]=true` (or specify `payment_method_types=['card']`).
  * Return the `client_secret` to the client.

* **Client (tiny Stripe.js usage; your UI everywhere else)**

  * Mount a single Card Element (it can look like your input).
  * Call `stripe.confirmCardPayment(client_secret, { payment_method: { card } })`.
  * Handle `requires_action` (3DS) if prompted (Stripe.js handles the challenge UI).

* **Server (webhook)**

  * Listen for `payment_intent.succeeded` / `payment_intent.payment_failed` to finalize your order.