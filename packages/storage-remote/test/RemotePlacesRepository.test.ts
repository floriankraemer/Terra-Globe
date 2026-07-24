import { InMemoryPlacesRepository } from "@terra-globe/core";
import { sharedRepositoryContractTests } from "@terra-globe/core/src/storage/repositoryContractTests.js";
import { RemotePlacesRepository } from "../src/RemotePlacesRepository.js";

sharedRepositoryContractTests(() => new RemotePlacesRepository(new InMemoryPlacesRepository()));
