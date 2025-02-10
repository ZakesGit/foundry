// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol"; 

contract MyToken is ERC20 {
    uint256 public constant INITIAL_SUPPLY = 1_000_000 * 1 ether;

    constructor() ERC20("MyToken", "MT"){
        _mint(msg.sender, INITIAL_SUPPLY);
    }
}
