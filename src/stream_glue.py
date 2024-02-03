import asyncio


async def forward(gen, sender):
    async for item in gen:
        should_continue = sender.send(item)

        if asyncio.iscoroutine(should_continue):
            should_continue = await should_continue

        if should_continue:
            continue
        else:
            break

    sender.close()
