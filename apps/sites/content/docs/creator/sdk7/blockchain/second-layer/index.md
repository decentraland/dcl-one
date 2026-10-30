> >-

# Second Layer

## About second layer solutions

Any transaction that affects the blockchain takes time to complete, and costs gas. Both these things are obstacles to making blockchain gaming popular, because players usually don't have the patience to wait that long for their actions to take effect, and aren't willing to spend money on many transactions as they play.

A common workaround is to keep most of the game-play off-chain, and only carry out blockchain transactions for key events, like earning a game item or registering a high score.

However, another way to overcome these limitations, as well as other scalability issues that are inherent to blockchains, is to rely on a _second layer_ blockchain, also called a _side-chain_.

A second layer is another blockchain that sits as an intermediary between a decentralized app and the main chain. This layer is more lightweight and can therefore provide faster responses and at a much lower gas cost.

Instead of making transactions directly into the main chain, transactions are done on the side chain, and then it's the duty of the side-chain to eventually sync these changes with the main chain.

The side-chain is able to provide faster responses because it's a smaller network with less nodes. The transactions that are carried out in the second layer are initially less secure, but they are eventually committed to the main chain in bulks, and all of the security checks of the main chain can be enforced there.

The side-chain is also able to lower individual transaction costs significantly because it groups many transactions into a single one when syncing with the main chain. So the gas that would need to be paid for one transaction on the main chain can be divided over several thousand transactions.

Also, when syncing with the main chain, several redundant transactions can be avoided entirely, further reducing transaction costs. For example, if Alice transfers 1 ETH to Bob, and then Bob transfers 1 ETH to Carol, then those two transactions can be simplified into one, by registering that Alice transfers directly to Carol.

Decentraland has a partnership with [Polygon](https://polygon.technology/), who provide their own side-chain on Ethereum. Their side-chain can be used by scenes to support fast and very cheap transactions.

## Try the Polygon Testnet

In order to use Polygon's Matic Testnet, you should first have:

* Ether in Sepolia network. You can obtain it for free from various external faucets like [this one](https://www.alchemy.com/faucets/ethereum-sepolia).

Then you must configure your Metamask account to include the Matic Testnet.

To do this:

1. Switch networks on Metamask by clicking on the name of the current network and selecting _Custom RPC_ at the bottom of the dropdown.
2. Fill in 'Polygon Amoy Testnet' as _Network name_, [**https://rpc-amoy.polygon.technology**](https://rpc-amoy.polygon.technology) as _New RPC URL_ and [**https://amoy.polygonscan.com**](https://amoy.polygonscan.com) as _Block Explorer URL_. For _Symbol_ set **POL**
3. Click 'Save' to add this new network to Metamask.


> [!WARNING]
> **📔 Note**: Learn more about how to set up Polygon on their [Documentation page](https://docs.polygon.technology/)

