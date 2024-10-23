import contractSdk from '../utils/contract';

(async function () {
    const contractAddress = 'inj1qe3lwunhcjgwupvckf6mkxllv9a76xkl0jqfyj';

    // const response = await contractSdk.execute(contractAddress, { deposit: {} }, [
    //     { amount: '1000000000000000000', denom: 'inj' },
    // ]);

    const response = await contractSdk.query(contractAddress, {
        user_info: { address: contractSdk.address },
    });

    console.log(response);
})();
